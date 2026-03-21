//! tileassetsgc.rs -- garbage collector for tile assets
//!
//! Remove entries from tile_assets table that do not appear in region_impostors table.
//!
//! Part of the Animats impostor system
//!
//!
//!     License: LGPL.
//!     Animats
//!     April, 2026.
//
use anyhow::{Error, anyhow};
use crate::{RegionData, RegionImpostorFaceData, TileAssetType};
use mysql::prelude::{Queryable};
use mysql::{PooledConn, params, Error::MySqlError};
//////use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One usage of a UUID in region_impostors.
/// This should be small, because we read in
/// all the assets for one grid.
struct UuidUsage {
    /// What asset is being used for
    pub asset_type: TileAssetType,
    /// Asset hash
    pub asset_hash: String,
    /// Region location (meters)
    pub region_loc: [u32;2],
    /// Region size (meters)
    pub region_size: [u32;2],
    /// Asset UUID
    pub asset_uuid: Uuid
}

/// The garbage collector for unused tiles.
pub struct TileGc {
    /// Which grid
    grid: String,
}

impl TileGc {
    ///  Usual new
    pub fn new(conn: &mut PooledConn, grid: &str) -> TileGc {
        Self {
            grid: grid.to_string(),
        }
    }
    
    /// Get all UUIDs in use.
    /// We have to do this the hard way, looking at each faces_json string,
    /// because this has to work with MySQL 8.0, where JSON capabiilties barely exist.
    /// In a more modern SQL implementation, we could do this whole job within SQL.
    fn get_uuids_in_use(&self, conn: &mut PooledConn) -> Result<Vec<UuidUsage>, Error> {
        const SELECT_UUIDS_SQL: &str = r"SELECT 
            region_loc_x, region_loc_y, region_size_x, region_size_y,
            sculpt_uuid, sculpt_hash, mesh_uuid, mesh_hash,
            faces_json        
        FROM region_impostors
            WHERE grid = :grid;";
 
        todo!();
    }
    
    /// Build the temporary table of UUIDs in use.
    fn build_temporary_table(&self, conn: &mut PooledConn, active_uuids: Vec<UuidUsage>) -> Result<(), Error> {
        const CREATE_INUSE_UUIDS_SQL: &str = r"CREATE TEMPORARY TABLE uuids_in_use (
            region_loc_x INT NOT NULL,
            region_loc_y INT NOT NULL,
            region_size_x INT NOT NULL,
            region_size_y INT NOT NULL,
            asset_type VARCHAR(20) NOT NULL,
            asset_uuid CHAR(36) DEFAULT NULL,
            asset_hash CHAR(8) NOT NULL)";
        const INSERT_INUSE_UUIDS_SQL: &str = r"INSERT INTO uuids_in_use
            (region_loc_x, region_loc_y, region_size_x, region_size_y, asset_type, asset_uuid, asset_hash)
            VALUES
            (:region_loc_x, :region_loc_y, :region_size_x, :region_size_y, :asset_type, :asset_uuid, :asset_hash)";
        //  Create the temporary table
        conn.query_drop(CREATE_INUSE_UUIDS_SQL)?;
        //  Get all the active UUIDs. In memory all at once, but under 1MB
        let active_uuids = self.get_uuids_in_use(conn)?;
        //  Put all the records in the temporary table.
        conn.exec_batch(
            INSERT_INUSE_UUIDS_SQL,
            active_uuids.iter().map(|p| params! {
                "region_loc_x" => p.region_loc[0],
                "region_loc_y" => p.region_loc[1],
                "region_size_x" => p.region_size[0],
                "region_size_y" => p.region_size[1],
                "asset_uuid" => p.asset_uuid.to_string(),
                "asset_hash" => p.asset_hash.clone(),
                "asset_type" => p.asset_type.to_str().to_string(),
            })
        )?;
        Ok(())
    }
    
    ///  Purge all unused tile assets
    fn purge_unused_tile_assets(&self, conn: &mut PooledConn) -> Result<(), Error> {
        const DELETE_UNUSED_TILE_ASSETS: &str = r"DELETE FROM TILE ASSETS WHERE
                ***NOT IN TEMPORARY TABLE***
        ";
        //  Create a temporary SQL table and insert all the UUIDs.
        //  This allows us to get MySQL to do the deletions.
        todo!();
    }
}
/*
/// MySQL error constants. Ought to be in some crate.
/// Duplicate entry
const ER_DUP_ENTRY: u16 = 1062;
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
                "RT" => Ok(Self::BaseTexture(Self::parse_texture_index(prefix)?)),
                "RE" => Ok(Self::EmissiveTexture(Self::parse_texture_index(prefix)?)),
                _ => Err(anyhow!("Invalid tile asset name prefix: {}", prefix))
            }
        }
    }
    
    /// The reverse operation, prefix from value
    pub fn to_prefix(&self) -> String {
        match self {
            Self::SculptTexture => "RS".to_string(),
            Self::Mesh => "RM".to_string(),
            Self::BaseTexture(n) => format!("RT{}", n),
            Self::EmissiveTexture(n) => format!("RE{}", n),
        }
    }
    
    /// Access to texture index
    pub fn get_texture_index(&self) -> Option<u8> {
        match self {
            Self::SculptTexture => None,
            Self::Mesh => None,
            Self::BaseTexture(n) => Some(*n),
            Self::EmissiveTexture(n) => Some(*n)
        }
    }
    
    /// To string, used with SQL.
    pub fn to_str(&self) -> &str {
        match self {
            Self::SculptTexture => "SculptTexture",
            Self::Mesh => "Mesh",
            Self::BaseTexture(_) => "BaseTexture",
            Self::EmissiveTexture(_) => "EmissiveTexture",
        }
    }
    
    /// Get one digit, with checking
    fn parse_texture_index(prefix: &str) -> Result<u8, Error> {
        if prefix.len() < 3 {
            Err(anyhow!("Too short tile asset name prefix: {}", prefix))
        } else {
            Ok(prefix[2..3].parse()?)
        }
    }
}

/// Parent of TerrainSculpt and TerrainMesh
pub trait TerrainGeometry {
    /// Get the tile asset type. Must be overridden.
    fn get_tile_type(&self) -> TileType;
    /// Get water height. Must be overridden
    fn get_water_height(&self) -> f32;
    /// Get adjusted (with skirt) scale and offset
    fn get_adjusted_scale_offset(&self) -> Result<(f32, f32), Error>;
}

/// What the LSL tool uploads for each uploaded impostor asset.
/// Intended for serde use.
#[derive(Deserialize, Clone, Debug)]
#[allow(dead_code)]
pub struct AssetUpload {
    /// Asset name - the name used in SL/OS
    pub asset_name: String,
    /// File name prefix. "RS", "RM", or RTn"
    /// Hash of asset content. Hex value.
    pub asset_hash: String,
    /// Region location (meters)
    pub region_loc: [u32;2],
    /// Region size (meters)
    pub region_size: [u32;2],
    /// Grid name
    pub grid: String,
    /// UUID of asset
    pub asset_uuid: Option<String>,
    /// Elevation offset - created, but never read
    elevation_offset: f32,
    /// Scale
    scale: [f32;3],
    /// Water height
    water_height: f32,
    /// Impostor LOD. 0 is highest level of detail.
    pub impostor_lod: u8,
    /// Tile assset type - derived from prefix
    pub tile_asset_type: TileAssetType,
}

impl AssetUpload {

    /// New, from available data. No UUID yet.
    /// UUID to be filled in later in uploadimpostor after upload of asset to SL/OS server.
    pub fn new(tile_asset_type: TileAssetType, region_data: &RegionData, terrain_geometry: &dyn TerrainGeometry, asset_hash: u32) -> Result<Self, Error> {
        let (zscale, elevation_offset) = terrain_geometry.get_adjusted_scale_offset()?;
        let sx = region_data.region_size_x;
        let sy = region_data.region_size_y;
        let sz = zscale;
        let scale = [sx as f32, sy as f32, sz];
        let water_height = terrain_geometry.get_water_height();
        
        let region_loc = [region_data.region_loc_x, region_data.region_loc_y];
        let region_size = [region_data.region_size_x, region_data.region_size_y];
        let grid = region_data.grid.clone();
        let impostor_lod = region_data.lod;
        let asset_uuid = None;
        //  Construct name that encodes the coords and hash. viz_group is no longer used.
        let asset_name = Self::impostor_name(&tile_asset_type.to_prefix(), region_data, terrain_geometry, impostor_lod, 0, asset_hash)?;
        
        Ok(Self {
            asset_name,
            asset_hash: hash_to_hex(asset_hash),
            region_loc,
            region_size,
            grid,
            asset_uuid,
            elevation_offset,
            scale,
            water_height,
            impostor_lod,
            tile_asset_type         
        })
    }
    
    /// Encoded name for impostor asset file.
    /// The name contains all the info we need to generate the impostor.
    /// viz_group_id is no longer used.
    /// Format: RS_x_y_sx_sy_sz_offset_lod_waterlevel_vizgroup_hash_
    fn impostor_name(
        prefix: &str,
        region: &RegionData,
        terrain_geometry: &dyn TerrainGeometry,
        lod: u8,
        viz_group_id: u32,
        hash: u32,
    ) -> Result<String, Error> {
        let x = region.region_loc_x;
        let y = region.region_loc_y;
        let (scale, offset) = terrain_geometry.get_adjusted_scale_offset()?;
        let sx = region.region_size_x;
        let sy = region.region_size_y;
        let sz = scale;
        let water_height = terrain_geometry.get_water_height();
        let s = format!("{}_{}_{}_{}_{}_{:.2}_{:.2}_{}_{}_{:.2}_{:08x}", prefix, x, y, sx, sy, sz, offset, lod, viz_group_id, water_height, hash);
        if s.len() > 63 {
            Err(anyhow!("Generated filename is too long: {}", s))
        } else {
            Ok(s)
        }
    }

    /// Create asset entry from asset name. 
    pub fn new_from_asset_name(asset_name: &str, grid: &str, asset_uuid: &str) -> Result<Self, Error> {
        //  Extract 11 fields from asset name.
        //  viz_group is no longer used but present as 0.
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
            water_height: fields[9].parse()?,
            asset_hash: fields[10].to_string(),
            asset_uuid: Some(Self::fix_uuid_string(asset_uuid)?),
            tile_asset_type: TileAssetType::new_from_prefix(fields[0])?,
        })
    }
    
    /// Construct from input JSON.
    pub fn new_from_asset_upload_short(upload_short: &AssetUploadShort) -> Result<Self, Error> {
        Self::new_from_asset_name(&upload_short.asset_name, &upload_short.grid, &upload_short.asset_uuid)
    }
    
    ///  Parse and check UUID
    fn fix_uuid_string(uuid_str: &str) -> Result<String, Error> {
        let uuid = Uuid::parse_str(uuid_str)?;
        Ok(uuid.to_string())
    }
    
    /// Insert a tile without a UUID. This is used before the asset has been created in the asset servers.
    /// Returns true if a new asset entry was created. False means this is a duplicate.
    pub fn insert_tile_without_uuid(&self, conn: &mut PooledConn, last_modified_opt: Option<DateTime<Utc>>) -> Result<bool, Error> {
        //  Insert if either nothing present, or matches on everything and creation time is old.
        //  Unique indicates for this table are:
        //   UNIQUE INDEX (grid, region_loc_x, region_loc_y, impostor_lod, asset_hash, texture_index, asset_type),
        //   UNIQUE INDEX (grid, asset_name)
        assert!(self.asset_uuid.is_none());  // must not have UUID yet.
        
        const SQL_GET_CREATION_TIME: &str = "SELECT creation_time FROM tile_assets
            WHERE grid = :grid AND region_loc_x = :region_loc_x AND impostor_lod = :impostor_lod AND texture_index = :texture_index AND asset_type = :asset_type";
             
        const SQL_INSERT_TILE: &str = r"INSERT INTO tile_assets
                (grid, region_loc_x, region_loc_y, region_size_x, region_size_y,
                impostor_lod, texture_index, asset_hash, asset_uuid,
                asset_name, asset_type,
                creation_time) 
            VALUES 
                (:grid, :region_loc_x, :region_loc_y, :region_size_x, :region_size_y,
                :impostor_lod, :texture_index, :asset_hash, :asset_uuid,
                :asset_name, :asset_type,
                :creation_time)";
        let creation_time = if let Some(last_modified) = last_modified_opt {
            last_modified
        } else {
            log::warn!("No last_modified timestamp, using current time.");
            Utc::now()
        };
        let params = params! {
            "grid" => self.grid.to_lowercase(),
            "asset_name" => self.asset_name.clone(),
            "asset_type" => self.tile_asset_type.to_str().to_string(),
            "region_loc_x" => self.region_loc[0],
            "region_loc_y" => self.region_loc[1],
            "region_size_x" => self.region_size[0],
            "region_size_y" => self.region_size[1],
            "impostor_lod" => self.impostor_lod,
            "texture_index" => self.tile_asset_type.get_texture_index(),
            "asset_uuid" => self.asset_uuid.clone(),
            "asset_hash" => self.asset_hash.clone(),
            "creation_time" => creation_time.naive_utc().to_string(),
        };
        log::debug!("Timestamp compare: last_modified: {:?}", last_modified_opt);
        if let Some(last_modified) = last_modified_opt {
            let naive_creation_time_opt: Option<NaiveDateTime> = conn.exec_first(SQL_GET_CREATION_TIME, &params)?;
            if let Some(naive_creation_time) = naive_creation_time_opt {
                let creation_time = DateTime::<Utc>::from_naive_utc_and_offset(naive_creation_time, Utc);
                if last_modified < creation_time {
                    log::warn!("Out of order asset info from server: {:?} is earlier than {:?} for {:?}",
                        last_modified, creation_time, params);
                    return Ok(false)
                }
            }
        }
        log::debug!("SQL tile asset creation: {:?}", params);
        //  Try the insert
        let result: Result<Option<usize>, _> = conn.exec_first(SQL_INSERT_TILE, &params);
        //  Check for duplicate type error as special case.
        if let Err(e) = result {
            if let MySqlError(ref sqerr) = e && sqerr.code == ER_DUP_ENTRY {
                log::debug!("Insert rejected, duplicate.");
                return Ok(false);
            }
            return Err(e.into())
        }
        Ok(true)
    }
    
    /// Add the UUID to a previously inserted tile.
    /// Returns true if a UUID was inserted. Returns false if no match.
    pub fn insert_uuid(&self, conn: &mut PooledConn) -> Result<bool, Error> {
        assert!(self.asset_uuid.is_some());
        const SQL_UPDATE_UUID: &str = r"
            UPDATE tile_assets 
            SET asset_uuid = :asset_uuid
            WHERE grid = :grid 
            AND asset_hash = :asset_hash
            AND region_loc_x = :region_loc_x 
            AND region_loc_y = :region_loc_y 
            AND region_size_x = :region_size_x
            AND region_size_y = :region_size_y
            AND impostor_lod = :impostor_lod 
            AND (texture_index = :texture_index OR (texture_index IS NULL AND :texture_index IS NULL))
            AND asset_type = :asset_type";
        let params = params! {
            "grid" => self.grid.to_lowercase(),
            "asset_type" => self.tile_asset_type.to_str().to_string(),
            "region_loc_x" => self.region_loc[0],
            "region_loc_y" => self.region_loc[1],
            "region_size_x" => self.region_size[0],
            "region_size_y" => self.region_size[1],
            "impostor_lod" => self.impostor_lod,
            "texture_index" => self.tile_asset_type.get_texture_index(),
            "asset_uuid" => self.asset_uuid.clone(),
            "asset_hash" => self.asset_hash.clone(),
        };
        log::debug!("Insert UUID params: {:?}", params);
        let _sink: Option<usize> = conn.exec_first(SQL_UPDATE_UUID, &params)?;
        let row_count = conn.affected_rows();
        //  ***NEED TO KNOW IF SUCCESS*** return true if insert changed a row.
        log::debug!("Tile asset UUID update succeeded. Rows: {:?}, params {:?}", row_count, params);
        Ok(row_count != 0)
    }
    
    /// Get asset UUID from tile_assets if already available.
    /// Vizgroup and index are not considered.
    pub fn get_asset_uuid(&self, conn: &mut PooledConn) -> Result<Option<Uuid>, Error> {
        const SQL_GET_ASSET_UUID: &str = r"SELECT asset_uuid FROM tile_assets 
            WHERE grid = :grid AND region_loc_x = :region_loc_x AND region_loc_y = :region_loc_y 
            AND region_size_x = :region_size_x AND region_size_y = :region_size_y
            AND asset_type = :asset_type AND asset_hash = :asset_hash
            AND asset_uuid IS NOT NULL";
        let params = params! {
            "grid" => self.grid.to_lowercase().clone(), 
            "region_loc_x" => self.region_loc[0],
            "region_loc_y" => self.region_loc[1],
            "region_size_x" => self.region_size[0],
            "region_size_y" => self.region_size[1],
            "asset_type" => self.tile_asset_type.to_str().to_string(),
            "asset_hash" => self.asset_hash.clone(),
            };
         log::debug!("get_asset_uuid params: {:?} from {:?}", params, self);
         let asset_uuids = conn.exec_map(
            SQL_GET_ASSET_UUID,
            params,
            |uuid : String| {
                log::debug!("get_asset_uuid result: {:?}", uuid);
                uuid
            })?;
        log::debug!("get_asset_uuid results: {:?}", asset_uuids);
        if asset_uuids.is_empty() {
            return Ok(None);
        }
        //  Found something
        if asset_uuids.len() > 1 {
            //  This is possible when viz_group numbers change, but is not fatal.
            log::warn!("Duplicate hashes for grid {} looking up {:?} asset at {:?} size {:?}", self.grid, self.tile_asset_type, self.region_loc, self.region_size);
        }
        let uuid_str = &asset_uuids[0];
        Ok(Some(Uuid::parse_str(uuid_str)?))
    }
    
    //  Look up region name.
    //  Returns name of region if exact match. Otherwise searches for
    //  some name in a larger area containing the region of interest.
    fn _look_up_region_name(conn: &mut PooledConn, grid: &str, loc: [u32;2], size: [u32;2]) -> Result<Option<String>, Error> {
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
        let names = conn.exec_map(
            SQL_GET_NAME,
            params,
            |(name, _region_loc_x, _region_loc_y) : (String, u32, u32)| {
            name
            })?;
        if names.is_empty() {
            Ok(None)
        } else {
            Ok(Some(names[0].clone()))
        }
    }
    
    //  Get face information, which is texture UUIDs.
    fn _get_faces_json(&mut self, conn: &mut PooledConn) -> Result<serde_json::Value, Error> {
        //  Get face texture data. One row for each face.
        const SQL_GET_TEXTURES: &str = r#"SELECT texture_index, asset_uuid, asset_hash, asset_type
            FROM tile_assets
            WHERE grid = :grid AND region_loc_x = :region_loc_x AND region_loc_y = :region_loc_y
                AND region_size_x = :region_size_x AND region_size_y = :region_size_y
                AND impostor_lod = :impostor_lod
                AND (asset_type = "BaseTexture" OR asset_type = "EmissiveTexture")
                AND asset_hash = :asset_hash)
            ORDER BY texture_index"#;
        let asset_upload = self;
        let texture_query_params = 
            params! {
                "grid" => asset_upload.grid.to_lowercase().clone(), 
                "region_loc_x" => asset_upload.region_loc[0],
                "region_loc_y" => asset_upload.region_loc[1],
                "region_size_x" => asset_upload.region_size[0],
                "region_size_y" => asset_upload.region_size[1],
                "impostor_lod" => asset_upload.impostor_lod,
                "asset_hash" => asset_upload.asset_hash.clone(),
            };
        log::debug!("Textures for sculpt/mesh {:?}, query params: {:?}", asset_upload.asset_name, texture_query_params);
        let texture_tuples = conn.exec_map(
            SQL_GET_TEXTURES,
            texture_query_params,
            |(texture_index, texture_uuid,texture_hash, asset_type) : (usize, String, String, String)| {
           (texture_index, texture_uuid, texture_hash, asset_type)
            },
        )?;        
        //  Build the textures as  JSON. Format is an array of JSON structs.        
        log::debug!("Textures for sculpt/mesh {:?}  {:?}", asset_upload.asset_name, texture_tuples);
        RegionImpostorFaceData::json_from_tuples(&texture_tuples)
    }

    /// All the info is already present in self. This just adds the UUID.
    pub fn update_tile(&mut self, conn: &mut PooledConn) -> Result<(), Error> {
        //  Update tile assets
        if let Some(asset_uuid_str) = &self.asset_uuid {
            //  Syntax check UUID.
            let _uuid = Uuid::parse_str(asset_uuid_str)?;
            let inserted = self.insert_uuid(conn)?;
            if !inserted {
                log::info!("Tile UUID unchanged for {:?}", self);
            }
        } else {
            return Err(anyhow!("Null UUID in update: {:?}", self));
        }
        Ok(())
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
*/
