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
use common::{RegionImpostorData};
use common::u8_to_elev;
use mysql::prelude::{Queryable};
use mysql::{Pool};
use mysql::{PooledConn, params};
use std::collections::HashMap;
use std::io::Write;
use serde::Deserialize;
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
        std::fs::File::create(LOG_FILE_NAME).expect("Unable to create log file"),
    )]);
    log::warn!("Logging to {:?}", LOG_FILE_NAME); // where the log is going
}

/// What the LSL tool uploads for each uploaded impostor asset.
/// Intended for serde use.
#[derive(Deserialize, Clone, Debug)]
pub struct AssetUpload {
    /// File name prefix. "RS", "RM", or RTn"
    prefix: String,
    /// Hash of asset content. Hex value.
    region_hash: String,
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
}

/// Array of impostor data as uploaded. This is what comes in as JSON.
pub type AssetUploadArray = Vec<AssetUpload>;

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

    /// SQL insert for new item
    fn do_sql_insert(
        &mut self,
        region_info: &AssetUploadArray,
        params: &HashMap<String, String>,
    ) -> Result<(), Error> {
/*
    
        const SQL_INSERT: &str = r"INSERT INTO raw_terrain_heights (grid, region_coords_x, region_coords_y, samples_x, samples_y, size_x, size_y, name, scale, offset, elevs,  water_level, creator) 
            VALUES
            (:grid, :region_coords_x, :region_coords_y, :samples_x, :samples_y, :size_x, :size_y, :name, :scale, :offset, :elevs, :water_level, :creator)";
        let creator = &self.owner_name
            .as_ref()
            .ok_or_else(|| anyhow!("No owner name from auth"))?;    // should fail upstream, not here.
        let samples = region_info.get_samples()?;
        let values = params! {
        //////"table" => RAW_TERRAIN_HEIGHTS,
        "grid" => region_info.grid.clone(),
        "region_coords_x" => region_info.region_coords[0],
        "region_coords_y" => region_info.region_coords[1],
        "size_x" => region_info.get_size()[0],
        "size_y" => region_info.get_size()[1],
        "name" => region_info.name.clone(),
        "scale" => region_info.scale,
        "offset" => region_info.offset,	
        "elevs" => region_info.get_elevs_as_blob()?,
        "samples_x" => samples[0],
        "samples_y" => samples[1],
        "water_level" => region_info.water_lev,
        "creator" => creator };
        log::debug!("SQL insert: {:?}", values);
        self.conn.exec_drop(SQL_INSERT, values)?;
        log::debug!("SQL insert succeeded.");
*/
        Ok(())
    }
    
    /// SQL insert for new item. Replaces entire record
    fn do_sql_full_update(
        &mut self,
        region_info: &RegionImpostorData,
        params: &HashMap<String, String>,
    ) -> Result<(), Error> {
/*
        const SQL_FULL_UPDATE: &str = r"UPDATE raw_terrain_heights 
            SET samples_x = :samples_x, samples_y = :samples_y, scale = :scale, offset = :offset, elevs = :elevs, water_level = :water_level, creator = :creator,
                size_x = :size_x, size_y = :size_y, name = :name, confirmation_time = NOW(), confirmer = NULL
            WHERE LOWER(grid) = :grid AND region_coords_x = :region_coords_x AND region_coords_y = :region_coords_y";           
        let creator = &self.owner_name
            .as_ref()
            .ok_or_else(|| anyhow!("No owner name from auth"))?;    // should fail upstream, not here.
        let samples = region_info.get_samples()?;
        let values = params! {
        "grid" => region_info.grid.clone(),
        "region_coords_x" => region_info.region_coords[0],
        "region_coords_y" => region_info.region_coords[1],
        "size_x" => region_info.get_size()[0],
        "size_y" => region_info.get_size()[1],
        "name" => region_info.name.clone(),
        "scale" => region_info.scale,
        "offset" => region_info.offset,	
        "elevs" => region_info.get_elevs_as_blob()?,
        "samples_x" => samples[0],
        "samples_y" => samples[1],
        "water_level" => region_info.water_lev,
        "creator" => creator };
        log::debug!("SQL update: {:?}", values);
        self.conn.exec_drop(SQL_FULL_UPDATE, values)?;
        log::debug!("SQL update succeeded.");
*/
        Ok(())
    }
/*    
    
    fn do_sql_confirmation_update(
        &mut self,
        region_info: &RegionImpostorData,
        params: &HashMap<String, String>,
    ) -> Result<(), Error> {
        const SQL_CONFIRMATION_UPDATE: &str = r"UPDATE raw_terrain_heights
            SET confirmation_time = NOW(), confirmer = :confirmer
            WHERE LOWER(grid) = :grid AND region_coords_x = :region_coords_x AND region_coords_y = :region_coords_y";           
        let confirmer = &self.owner_name
            .as_ref()
            .ok_or_else(|| anyhow!("No owner name from auth"))?;    // should fail upstream, not here.
        let values = params! {
        "grid" => region_info.grid.clone(),
        "region_coords_x" => region_info.region_coords[0],
        "region_coords_y" => region_info.region_coords[1],
        "confirmer" => confirmer };
        log::debug!("SQL confirmation update: {:?}", values);
        self.conn.exec_drop(SQL_CONFIRMATION_UPDATE, values)?;
        log::debug!("SQL confirmation update succeeded.");
        Ok(())
    }
    
    /// Is this a duplicate?
    fn do_sql_unchanged_check(
        &mut self,
        region_info: &RegionImpostorData,
    ) -> Result<ChangeStatus, Error> {
        
        let samples = region_info.get_samples()?;
        let grid = &region_info.grid;
        let region_coords_x = region_info.region_coords[0];
        let region_coords_y = region_info.region_coords[1];
        let new_elevs= region_info.get_elevs_as_blob()?;
        const SQL_SELECT: &str = r"SELECT size_x, size_y, samples_x, samples_y, scale, offset, elevs, name, water_level
            FROM raw_terrain_heights
            WHERE LOWER(grid) = :grid AND region_coords_x = :region_coords_x AND region_coords_y = :region_coords_y";
        let is_sames = self.conn.exec_map(
            SQL_SELECT,
            params! { grid, region_coords_x, region_coords_y },
            |(size_x, size_y, samples_x, samples_y, scale, offset, elevs, name, water_level) : (u32, u32, u32, u32, f32, f32, Vec<u8>, String, f32)| {
                //  Is the stored data identical to what we just read from the region?
                log::trace!("Elevs:\n{:?} vs\n{:?}", elevs, new_elevs); // ***TEMP***
                let is_same = 
                    size_x == region_info.get_size()[0] && 
                    size_y == region_info.get_size()[1] &&
                    samples_x == samples[0] && 
                    samples_y == samples[1] &&
                    (scale - region_info.scale).abs() < Self::ELEV_ERROR_TOLERANCE  &&
                    (offset - region_info.offset).abs() < Self::ELEV_ERROR_TOLERANCE &&
                    Self::check_elev_err_within_tolerance(&elevs, &new_elevs, scale, offset, Self::ELEV_ERROR_TOLERANCE) &&
                    name == region_info.name &&
                    water_level == region_info.water_lev;                    
                is_same
            },
        )?;
        //  Changed?
        Ok(if is_sames.is_empty() {
            ChangeStatus::None
        } else {
            //  Must be 1, because of SELECT on unique key.
            assert!(is_sames.len() == 1);
            if is_sames[0] {
                ChangeStatus::NoChange
            } else {
                ChangeStatus::Changed
            }
        })
    }
*/

    /// Fix up some fields with strange formatting
    /// Texture ID prefix will be "XXn", where the first two characters indicate the type of texture.
    fn get_texture_index(prefix: &str) -> Result<u8, Error> {
        Ok(prefix[2..].parse()?)
    }
    
    /// Hash strings are hex strings. 
    /// We want the hash without any prefix, as 16 chars.
    fn fix_hash_string(hash_str: &str) -> Result<String, Error> {
        let without_prefix = hash_str.trim_start_matches("0x");
        let z = u64::from_str_radix(without_prefix, 16)?;
        Ok(format!("{:16x}", z))
    }
    
    //  Parse and check UUID
    fn fix_uuid_string(uuid_str: &str) -> Result<String, Error> {
        let uuid = Uuid::parse_str(uuid_str)?;
        Ok(uuid.to_string())
    }

    /// Update terrain tile. A new terrain tile has been added, and needs to be added to the database.
    /*
        name VARCHAR(100) NOT NULL,
    region_loc_x INT NOT NULL,
    region_loc_y INT NOT NULL,
    region_size_x INT NOT NULL,
    region_size_y INT NOT NULL,
    impostor_lod TINYINT NOT NULL,
    viz_group INT NOT NULL,
    texture_index SMALLINT NOT NULL,
    texture_uuid CHAR(36) NOT NULL,  
    texture_hash CHAR(16) NOT NULL,
    creation_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    */
    fn update_terrain_tile(&mut self, asset_upload: &AssetUpload) -> Result<(), Error> {
/*
        const OLD_SQL_UPDATE_TILE: &str = r"UPDATE tile_textures
            SET grid = :grid, region_coords_x = :region_coords_x,region_coords_y = :region_coords_y,
                region_size_x = :region_size_x, region_size_y = :region_size_y, impostor_lod = :impostor_lod,
                viz_group = :viz_group, texture_index = :texture_index, texture_hash = :texture_hash, 
                texture_uuid = :texture_uuid, 
                creation_time = NOW()            
            WHERE grid = :grid 
                AND region_coords_x = :region_coords_x AND region_coords_y = :region_coords_y
                AND region_size_x = :region_size_x AND region_size_y = :region_size_y
                AND viz_group = :viz_group AND 
                AND impostor_lod = :impostor_lod AND texture_index = :texture_index";
*/
            //  Insert tile, or update hash and uuid if exists. 
            const SQL_UPDATE_TILE: &str = r"INSERT INTO tile_textures
                (grid, region_coords_x, region_coords_y, region_size_x, region_size_y,
                impostor_lod, viz_group, texture_index, texture_hash, texture_uuid,
                creation_time) 
            VALUES 
                (:grid, :region_coords_x, :region_coords_y, :region_size_x, :region_size_y,
                :impostor_lod, :viz_group, :texture_index, :texture_hash, :texture_uuid,
                NOW()) 
            ON DUPLICATE KEY UPDATE
                texture_hash = :texture_hash, texture_uuid = :texture_uuid, creation_time = NOW()";
        //  UNIQUE INDEX (grid, region_loc_x, region_loc_y, impostor_lod, viz_group, texture_index)
        let values = params! {
            "grid" => asset_upload.grid.to_lowercase(),
            "region_loc_x" => asset_upload.region_loc[0],
            "region_loc_y" => asset_upload.region_loc[1],
            "region_size_x" => asset_upload.region_size[0],
            "region_size_y" => asset_upload.region_size[1],
            "impostor_lod" => asset_upload.impostor_lod,
            "viz_group" => asset_upload.viz_group,
            "texture_index" => Self::get_texture_index(&asset_upload.prefix)?,
            "texture_uuid" => Self::fix_uuid_string(&asset_upload.asset_uuid)?,
            "texture_hash" => Self::fix_hash_string(&asset_upload.region_hash)?,
        };
        log::debug!("SQL terrain tile update: {:?}", values);
        self.conn.exec_drop(SQL_UPDATE_TILE, values)?;
        log::debug!("SQL terrain tile update succeeded.");
        Ok(())
    }
    
    /// Parse a request
    fn parse_request(
        b: &[u8],
        _env: &HashMap<String, String>,
    ) -> Result<AssetUploadArray, Error> {
        //  Should be UTF-8. Check.
        let s = core::str::from_utf8(b)?;
        if s.trim().is_empty() {
            return Err(anyhow!("Empty request. JSON was expected"));
        }
        log::info!("Uploaded JSON:\n{}", s);
        //  Should be valid JSON
        let parsed: AssetUploadArray = serde_json::from_str(s)?;
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
        asset_info: AssetUploadArray,
        params: &HashMap<String, String>,
    ) -> Result<(usize, String), Error> {
        //  We have an array of assets.
        log::info!("Processing {} assets.", asset_info.len());
        for asset_upload in &asset_info {
            match &asset_upload.prefix[0..2] {
                "RS" => {
                    //  Sculpt
                    log::debug!("Sculpts not implemented yet");
                }
                "RT" => {
                    //  Texture
                    self.update_terrain_tile(asset_upload)?;
                }
                _ => { 
                    return Err(anyhow!("Invalid asset upload prefix: {}", asset_upload.prefix));
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

