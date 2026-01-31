//! Upload Second Life / Open Simulator asset info to server
//! Part of the Animats impostor system
//!
//! At this point, the asset exists on the SL/OS asset store.
//! A script running in an SL/OS viewer calls this service to tell it about new assets.
//!
//!     License: LGPL.
//!     Animats
//!     August, 2025.
//
#![forbid(unsafe_code)]
use anyhow::{Error, anyhow};
use log::LevelFilter;
use common::Credentials;
use common::init_fcgi;
use common::{Handler, Request, Response};
use common::{RegionImpostorFaceData};
use mysql::prelude::{Queryable};
use mysql::{Pool};
use mysql::{PooledConn, params};
use std::collections::HashMap;
use std::io::Write;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
mod auth;
use auth::{Authorizer, AuthorizeType};

/// MySQL Credentials for uploading.
/// This filename will be searched for in parent directories,
/// so it can be placed above the web root, where the web server can't see it.
/// The upload credentials file must contain
///
///     DB_USER = username
///     DB_PASS = databasepassword
///     DB_HOST = hostname
///     DB_PORT = portnumber (optional, defaults to 3306)
///     DB_NAME = databasename
///
const UPLOAD_CREDS_FILE: &str = "upload_credentials.txt";

/// Debug logging
fn logger() {
    //  Log file is openly visible as a web page.
    //  Only for debug tests.
    const LOG_FILE_NAME: &str = "logs/uploadimpostorlog.txt";
    let _ = simplelog::CombinedLogger::init(vec![simplelog::WriteLogger::new(
        LevelFilter::Debug,
        simplelog::Config::default(),
        ////std::fs::File::create(LOG_FILE_NAME).expect("Unable to create log file"),
        std::fs::OpenOptions::new().create(true).append(true).open(LOG_FILE_NAME).expect("Unable to create log file"),
    )]);
    log::warn!("Logging to {:?}", LOG_FILE_NAME); // where the log is going
}

/// Asset type
#[derive(Clone, Debug, PartialEq, PartialOrd, Deserialize, Serialize)]
pub enum TileAssetType {
    /// Base color of tile
    BaseTexture(u8), 
    /// Emissive texture of tile
    EmissiveTexture(u8),
    /// Geometry as a sculpt texture
    SculptTexture,
    /// Mesh (future)
    Mesh
}

impl TileAssetType {
    /// From filename prefix string. Valid prefix values are RS, RM, RTn, and REn.
    pub fn new_from_prefix(prefix: &str) -> Result<Self, Error> {
        if prefix.len() < 2 {
            Err(anyhow!("Too short tile asset name prefix: {}", prefix))
        } else {
            match &prefix[0..2] {
                "RS" => Ok(Self::SculptTexture),
                "RM" => Ok(Self::Mesh),
                "RT" => Ok(Self::BaseTexture(Self::get_texture_index(prefix)?)),
                "RE" => Ok(Self::EmissiveTexture(Self::get_texture_index(prefix)?)),
                _ => Err(anyhow!("Invalid tile asset name prefix: {}", prefix))
            }
        }
    }
    
    /// Get one digit, with checking
    fn get_texture_index(prefix: &str) -> Result<u8, Error> {
        if prefix.len() < 3 {
            Err(anyhow!("Too short tile asset name prefix: {}", prefix))
        } else {
            Ok(prefix[2..3].parse()?)
        }
    }
}

/// What the LSL tool uploads for each uploaded impostor asset.
/// Intended for serde use.
#[derive(Deserialize, Clone, Debug)]
pub struct AssetUpload {
    /// Asset name - the name used in SL/OS
    asset_name: String,
    /// File name prefix. "RS", "RM", or RTn"
    /// Hash of asset content. Hex value.
    asset_hash: String,
    /// Region location (meters)
    region_loc: [u32;2],
    /// Region size (meters)
    region_size: [u32;2],
    /// Grid name
    grid: String,
    /// UUID of asset
    asset_uuid: String,
    /// Elevation offset 
    elevation_offset: f32,
    /// Scale
    scale: [f32;3],
    /// Water height
    water_height: f32,
    /// Impostor LOD. 0 is highest level of detail.
    impostor_lod: u8,
    /// Visibility group - only one viz group at a time is visible
    viz_group: u32,
    /// Tile assset type - derived from prefix
    tile_asset_type: TileAssetType,
}

impl AssetUpload {
    pub fn new_from_asset_name(asset_name: &str, grid: &str, asset_uuid: &str) -> Result<Self, Error> {
        //  Extract 11 fields from asset name
        const FIELD_COUNT: usize = 11;
        let fields: Vec<&str> = asset_name.split('_').collect();
        if fields.len() != FIELD_COUNT {
            return Err(anyhow!("Asset name did not contain {} fields: {}", FIELD_COUNT, asset_name));
        }
        Ok(Self {
            grid: grid.to_string(),
            asset_name: asset_name.to_string(),
            region_loc: [fields[1].parse()?, fields[2].parse()?],
            region_size: [fields[3].parse()?, fields[4].parse()?],
            scale: [fields[3].parse()?, fields[4].parse()?, fields[5].parse()?],
            elevation_offset: fields[6].parse()?,
            impostor_lod: fields[7].parse()?,
            viz_group: fields[8].parse()?,
            water_height: fields[9].parse()?,
            asset_hash: fields[10].to_string(),
            asset_uuid: Self::fix_uuid_string(asset_uuid)?,
            tile_asset_type: TileAssetType::new_from_prefix(fields[0])?,
        })
    }
    
    /// Construct from input JSON.
    fn new_from_asset_upload_short(upload_short: &AssetUploadShort) -> Result<Self, Error> {
        Self::new_from_asset_name(&upload_short.asset_name, &upload_short.grid, &upload_short.asset_uuid)
    }
    
    ///  Parse and check UUID
    fn fix_uuid_string(uuid_str: &str) -> Result<String, Error> {
        let uuid = Uuid::parse_str(uuid_str)?;
        Ok(uuid.to_string())
    }
}

/// Short version of asset upload.
/// For serde use.
/// This is what the client sends us as JSON.
#[derive(Deserialize, Clone, Debug)]
pub struct AssetUploadShort {
    /// Asset name - the name used in SL/OS.
    /// This encodes all the other fields.
    /// It's the only way we can attach metadata to SL/OS content.
    asset_name: String,
    /// UUID of asset
    asset_uuid: String,
    /// Grid name
    grid: String,
}

/// Array of impostor data as uploaded. This is what comes in as JSON.
pub type AssetUploadArrayShort = Vec<AssetUploadShort>;

///  Our handler

struct AssetUploadHandler {
    /// MySQL onnection pool. We only use one.
    #[allow(dead_code)] // needed to keep the pool alive, but never referenced.
    pool: Pool,
    /// Active MySQL connection.
    conn: PooledConn,
    /// Owner of object at other end
    owner_name: Option<String>,
}
impl AssetUploadHandler {

    /// Usual new. Saves connection pool for use.
    pub fn new(pool: Pool) -> Result<Self, Error> {
        let conn = pool.get_conn()?;
        Ok(Self { pool, conn, owner_name: None  })
    }

    /// Fix up some fields with strange formatting
    /// Texture ID prefix will be "XXn", where the first two characters indicate the type of texture.
    fn get_texture_index(prefix: &str) -> Result<u8, Error> {
        Ok(prefix[2..].parse()?)
    }
    
    /// Hash strings are hex strings. 
    /// We want the hash without any prefix, as 8 chars.
    fn fix_hash_string(hash_str: &str) -> Result<String, Error> {
        let without_prefix = hash_str.trim_start_matches("0x");
        let z = u32::from_str_radix(without_prefix, 16)?;
        Ok(format!("{:08x}", z))
    }
    
    /// Update terrain tile. A new terrain tile has been added, and needs to be added to the database.
    fn update_mesh_tile(&mut self, asset_upload: &AssetUpload) -> Result<(), Error> {
        Err(anyhow!("Mesh tiles unimplemented"))
    }

    /// Update terrain tile. A new terrain tile has been added, and needs to be added to the database.
    fn update_texture_tile(&mut self, asset_upload: &AssetUpload, texture_index: u8, asset_type: &str) -> Result<(), Error> {
        //  Only insert textures here, not sculpts or meshes.
        assert!(asset_type == "BaseTexture" || asset_type == "EmissiveTexture");
        //  Insert tile, or update hash and uuid if exists. 
        const SQL_UPDATE_TILE: &str = r"INSERT INTO tile_assets
                (grid, region_loc_x, region_loc_y, region_size_x, region_size_y,
                impostor_lod, viz_group, texture_index, asset_hash, asset_uuid,
                asset_name, asset_type,
                creation_time) 
            VALUES 
                (:grid, :region_loc_x, :region_loc_y, :region_size_x, :region_size_y,
                :impostor_lod, :viz_group, :texture_index, :asset_hash, :asset_uuid,
                :asset_name, :asset_type,
                NOW()) 
            ON DUPLICATE KEY UPDATE
                asset_hash = :asset_hash, asset_uuid = :asset_uuid, creation_time = NOW()";
        //  UNIQUE INDEX (grid, region_loc_x, region_loc_y, impostor_lod, viz_group, texture_index)
        let params = params! {
            "grid" => asset_upload.grid.to_lowercase(),
            "asset_name" => asset_upload.asset_name.clone(),
            "asset_type" => asset_type,
            "region_loc_x" => asset_upload.region_loc[0],
            "region_loc_y" => asset_upload.region_loc[1],
            "region_size_x" => asset_upload.region_size[0],
            "region_size_y" => asset_upload.region_size[1],
            "impostor_lod" => asset_upload.impostor_lod,
            "viz_group" => asset_upload.viz_group,
            "texture_index" => texture_index,
            "asset_uuid" => asset_upload.asset_uuid.clone(),
            "asset_hash" => asset_upload.asset_hash.clone(),
        };
        log::debug!("SQL terrain tile update: {:?}", params);
        self.conn.exec_drop(SQL_UPDATE_TILE, params)?;
        log::debug!("SQL terrain tile update succeeded.");
        Ok(())
    }
    
    //  Look up region name.
    //  Returns name of region if exact match. Otherwise searches for
    //  some name in a larger area containing the region of interest.
    fn look_up_region_name(&mut self, grid: &str, loc: [u32;2], size: [u32;2]) -> Result<Option<String>, Error> {
        //  Look up some name in the rectangle of interest.
        //  For LOD 0, this gets the region of interest.
        //  For lower LODs, the corner might be a nameless water region, so we pick some region in the rectangle.
        const SQL_GET_NAME: &str = r"SELECT name, region_loc_x, region_loc_y
            FROM raw_terrain_heights
            WHERE region_loc_x >= :region_loc_x AND region_loc_y >= :region_loc_y
            AND region_loc_x <= :region_loc_x + :region_size_x
            AND region_loc_y <= :region_loc_y + :region_size_y
            ORDER BY region_loc_x, region_loc_y LIMIT 1";
        let params = params! {
            "grid" => grid.to_lowercase().clone(), 
            "region_loc_x" => loc[0],
            "region_loc_y" => loc[1],
            "region_size_x" => size[0],
            "region_size_y" => size[1],
            };
        let names = self.conn.exec_map(
            SQL_GET_NAME,
            params,
            |(name, region_loc_x, region_loc_y) : (String, u32, u32)| {
            name
            })?;
        if names.is_empty() {
            Ok(None)
        } else {
            Ok(Some(names[0].clone()))
        }
    }
    
    /// Update a sculpt tile.
    /// ***NEED TO UPLOAD TO tile_asset table***
    fn update_sculpt_tile(&mut self, asset_upload: &AssetUpload) -> Result<(), Error> {
        //  Most of the info we need is in asset_upload, but we also need:
        //  - name
        //  - face texture data.
        log::debug!("Update sculpt tile: {:?}", asset_upload);
        //  Get face texture data. One row for each face.
        const SQL_GET_TEXTURES: &str = r#"SELECT texture_index, asset_uuid, asset_hash, asset_type
            FROM tile_assets
            WHERE grid = :grid AND region_loc_x = :region_loc_x AND region_loc_y = :region_loc_y
                AND region_size_x = :region_size_x AND region_size_y = :region_size_y
                AND viz_group = :viz_group AND impostor_lod = :impostor_lod
                AND (asset_type = "BaseTexture" OR asset_type = "EmissiveTexture")
            ORDER BY texture_index"#;
        let texture_query_params = 
            params! {
                "grid" => asset_upload.grid.to_lowercase().clone(), 
                "region_loc_x" => asset_upload.region_loc[0],
                "region_loc_y" => asset_upload.region_loc[1],
                "region_size_x" => asset_upload.region_size[0],
                "region_size_y" => asset_upload.region_size[1],
                "impostor_lod" => asset_upload.impostor_lod,
                "viz_group" => asset_upload.viz_group,
            };
        let name_opt = self.look_up_region_name(&asset_upload.grid.to_lowercase(), asset_upload.region_loc, asset_upload.region_size, )?;
        //  Name is only for debug and documentation
        let name = if let Some(name) = name_opt { name } else { "(UNKNOWN)".to_string() };
        log::debug!("Textures for sculpt {:?}, query params: {:?}", name, texture_query_params);
        let texture_tuples = self.conn.exec_map(
            SQL_GET_TEXTURES,
            texture_query_params,
            |(texture_index, texture_uuid,texture_hash, asset_type) : (usize, String, String, String)| {
           (texture_index, texture_uuid, texture_hash, asset_type)
            },
        )?;        
        //  Build the textures as  JSON. Format is an array of JSON structs.        
        log::debug!("Textures for sculpt {:?}  {:?}", name, texture_tuples);
        let faces_json = RegionImpostorFaceData::json_from_tuples(&texture_tuples)?;
        //  We have all the info now. Update the region_impostor table.
        //  Insert tile, or update hash and uuid if exists. 
        const SQL_IMPOSTOR: &str = r"INSERT INTO region_impostors
                (grid, name, region_loc_x, region_loc_y, region_size_x, region_size_y, uniqueness_viz_group,
                scale_x, scale_y, scale_z, 
                elevation_offset, impostor_lod, viz_group, 
                sculpt_uuid,
                water_height, creation_time, faces_json) 
            VALUES 
                (:grid, :name, :region_loc_x, :region_loc_y, :region_size_x, :region_size_y, :uniqueness_viz_group,
                :scale_x, :scale_y, :scale_z,
                :elevation_offset, :impostor_lod, :viz_group, 
                :sculpt_uuid, 
                :water_height, NOW(), faces_json)
            ON DUPLICATE KEY UPDATE
                scale_x = :scale_x, scale_y = :scale_y, scale_z = :scale_z,
                elevation_offset = :elevation_offset, impostor_lod := impostor_lod, viz_group = :viz_group,
                sculpt_uuid = :sculpt_uuid,
                water_height = :water_height, creation_time = NOW(), faces_json = :faces_json";
               
        let insert_params = params! {
                "grid" => asset_upload.grid.to_lowercase().clone(),
                "name" => name,
                "sculpt_uuid" => asset_upload.asset_uuid.clone(),
                "region_loc_x" => asset_upload.region_loc[0],
                "region_loc_y" => asset_upload.region_loc[1],
                "region_size_x" => asset_upload.region_size[0],
                "region_size_y" => asset_upload.region_size[1],
                "scale_x" => asset_upload.scale[0], // ***CONVERT TO INT***
                "scale_y" => asset_upload.scale[1], // ***CONVERT TO INT***
                "scale_z" => asset_upload.scale[2],
                "impostor_lod" => asset_upload.impostor_lod,
                "uniqueness_viz_group" => asset_upload.viz_group, // ***NOT SURE ABOUT THIS***
                "viz_group" => asset_upload.viz_group,
                "elevation_offset" => asset_upload.elevation_offset,
                "water_height" => asset_upload.water_height,
                "faces_json" => faces_json.to_string(),
            };
        //  Finally insert into the impostor table
        log::debug!("Inserting impostor into region_impostors, params: {:?}", insert_params);
        self.conn.exec_drop(SQL_IMPOSTOR, insert_params)?;
        Ok(())
    }
    
    /// Parse a request
    fn parse_request(
        b: &[u8],
        _env: &HashMap<String, String>,
    ) -> Result<AssetUploadArrayShort, Error> {
        //  Should be UTF-8. Check.
        let s = core::str::from_utf8(b)?;
        if s.trim().is_empty() {
            return Err(anyhow!("Empty request. JSON was expected"));
        }
        log::info!("Uploaded JSON:\n{}", s);
        //  Should be valid JSON
        let parsed: AssetUploadArrayShort = serde_json::from_str(s)?;
        Ok(parsed)
    }

    /// Handle request.
    ///
    /// Start a database transaction.
    /// Check if this data is the same as any stored data for this region.
    /// If yes, just update confirmation user and time.
    /// If no, replace old data entirely.
    fn process_request(
        &mut self,
        asset_info_short: AssetUploadArrayShort,
        params: &HashMap<String, String>,
    ) -> Result<(usize, String), Error> {
        //  We have an array of assets.
        log::info!("Processing {} assets.", asset_info_short.len());
        for asset_upload_short in &asset_info_short {
            let asset_upload = AssetUpload::new_from_asset_upload_short(asset_upload_short)?;
            match &asset_upload.tile_asset_type {
                TileAssetType::SculptTexture => {
                    //  Sculpt
                    self.update_sculpt_tile(&asset_upload)?;
                }
                TileAssetType::Mesh => {
                    //  Texture
                    self.update_mesh_tile(&asset_upload)?;
                }
                TileAssetType::BaseTexture(ix) => {
                    //  Texture
                    self.update_texture_tile(&asset_upload, *ix, "BaseTexture")?;
                }
                TileAssetType::EmissiveTexture(ix) => {
                    //  Texture
                    self.update_texture_tile(&asset_upload, *ix, "EmissiveTexture")?;
                }
            }
        }
        Ok((200, "Asset upload successful".to_string()))
    }
}
//  Our "handler"
impl Handler for AssetUploadHandler {
    fn handler(
        &mut self,
        out: &mut dyn Write,
        request: &Request,
        env: &HashMap<String, String>,
    ) -> Result<(), Error> {
        //  We have a request. It's supposed to be in JSON.
        //  Parse. Error 400 with message if fail.
        match Self::parse_request(&request.standard_input, env) {
            Ok(req) => {
                log::info!("Request made: {:?} env {:?}", req, env);
                let params = request
                    .params
                    .as_ref()
                    .ok_or_else(|| anyhow!("No HTTP parameters found"))?;
                //  This must be a POST
                if let Some(request_method) = params.get("REQUEST_METHOD") {  
                    if request_method.to_uppercase().trim() != "POST" {             
                        return Err(anyhow!("Request method \"{}\" was not POST.", request_method));
                    }
                } else {
                    return Err(anyhow!("No HTTP request method."));
                }
                //  Authorize
                self.owner_name = Some(Authorizer::authorize(AuthorizeType::UploadImpostors, env, params)?);
                //  Process. Error 500 if fail.
                match self.process_request(req, &params) {
                    Ok((status, msg)) => {
                        //  Success. Send a plain "OK"
                        let http_response = Response::http_response("text/plain", status, "OK");
                        //  Return something useful.
                        let b = msg.into_bytes();
                        Response::write_response(out, request, http_response.as_slice(), &b)?;
                    }
                    Err(e) => {
                        let http_response = Response::http_response(
                            "text/plain",
                            500,
                            format!("Problem processing request: {:?}", e).as_str(),
                        );
                        Response::write_response(out, request, http_response.as_slice(), &[])?;
                    }
                }
            }
            Err(e) => {
                let http_response = Response::http_response(
                    "text/plain",
                    400,
                    format!("Incorrect request: {:?}", e).as_str(),
                );
                //  Return something useful.
                //////let b = format!("Env: {:?}\nParams: {:?}\n", env, request.params).into_bytes();
                let b = [];
                Response::write_response(out, request, http_response.as_slice(), &b)?;
            }
        }
        Ok(())
    }
}

/// Run the responder.
pub fn run_responder() -> Result<(), Error> {
    log::info!("Environment: {:?}", std::env::vars());
    //  Set up in and out sockets.
    //  Communication with the parent process is via a UNIX socket.
    //  This is a pain to set up, because UNIX sockets are badly mis-matched
    //  to parent/child process communication.
    //  See init_fcgi for how it is done.
    let listener = init_fcgi()?;
    //  Accept a connection on the listener socket. This hooks up
    //  input and output to the parent process.
    let (socket, _addr) = listener.accept()?;
    let outsocket = socket.try_clone()?;
    let mut instream = std::io::BufReader::new(socket);
    let mut outio = std::io::BufWriter::new(outsocket);
    //  Connect to the database
    let creds = Credentials::new(UPLOAD_CREDS_FILE)?;
    //  Optional MySQL port number
    let portnum = if let Some(port) = creds.get("DB_PORT") {
        port.parse::<u16>()?
    } else {
        //  Use MySQL default
        3306
    };
    let opts = mysql::OptsBuilder::new()
        //  Dreamhost is still using old authentication
        .secure_auth(false)
        .ip_or_hostname(creds.get("DB_HOST"))
        .tcp_port(portnum)
        .user(creds.get("DB_USER"))
        .pass(creds.get("DB_PASS"))
        .db_name(creds.get("DB_NAME"));
    drop(creds);
    //////log::info!("Opts: {:?}", opts);
    let pool = Pool::new(opts)?;
    log::info!("Connected to database.");
    let mut asset_upload_handler = AssetUploadHandler::new(pool)?;
    //  Run the FCGI server.
    common::run(&mut instream, &mut outio, &mut asset_upload_handler)
}

/// Main program
pub fn main() {
    logger();
    match run_responder() {
        Ok(()) => {}
        Err(e) => {
            log::error!("Upload server failed: {:?}", e);
            panic!("Upload server failed: {:?}", e);
        }
    }
}

