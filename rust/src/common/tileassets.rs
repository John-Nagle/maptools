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
use anyhow::{Error, anyhow};
use log::LevelFilter;
use crate::{RegionData, HeightField, RegionImpostorFaceData};
use mysql::prelude::{Queryable};
use mysql::{Pool, TxOpts, PooledConn, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    asset_uuid: Option<String>,
    /// Elevation offset 
    elevation_offset: f32,
    /// Scale
    scale: [f32;3],
    /// Water height
    water_height: f32,
    /// Impostor LOD. 0 is highest level of detail.
    impostor_lod: u8,
    /// Tile assset type - derived from prefix
    pub tile_asset_type: TileAssetType,
}

impl AssetUpload {

    /// New, from available data.
    /// UUID is optional so that we can create tile_asset rows with a UUID to be filled in later.
    pub fn new(tile_asset_type: TileAssetType, region_data: &RegionData, height_field: &HeightField, impostor_lod: u8, asset_hash: u32, asset_uuid: Option<String>) -> Result<Self, Error> {
        let x = region_data.region_loc_x;
        let y = region_data.region_loc_y;
        let (zscale, elevation_offset) = height_field.get_scale_offset()?;
        let sx = region_data.region_size_x;
        let sy = region_data.region_size_y;
        let sz = zscale;
        let scale = [sx as f32, sy as f32, sz];
        let water_height = height_field.water_level;
        
        let region_loc = [region_data.region_loc_x, region_data.region_loc_y];
        let region_size = [region_data.region_size_x, region_data.region_size_y];
        let grid = region_data.grid.clone();
        let asset_name = "???".to_string();   // ***TEMP*** Need to generate name
        
        Ok(Self {
            asset_name,
            asset_hash: asset_hash.to_string(),
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
    /// Format: RS_x_y_sx_sy_sz_offset_lod_waterlevel_vizgroup_hash_
    fn impostor_name(
        prefix: &str,
        region: &RegionData,
        height_field: &HeightField,
        lod: u8,
        viz_group_id: u32,
        hash: u32,
    ) -> Result<String, Error> {
        let x = region.region_loc_x;
        let y = region.region_loc_y;
        let (scale, offset) = height_field.get_scale_offset()?;
        let sx = region.region_size_x;
        let sy = region.region_size_y;
        let sz = scale;
        let water_level = height_field.water_level;
        //////Ok(format!("{}_{}_{}_{}_{}_{:.2}_{:.2}_{}_{}_{:.2}_0x{:016x}", prefix, x, y, sx, sy, sz, offset, lod, viz_group_id, water_level, hash))
        let s = format!("{}_{}_{}_{}_{}_{:.2}_{:.2}_{}_{}_{:.2}_{:08x}", prefix, x, y, sx, sy, sz, offset, lod, viz_group_id, water_level, hash);
        if s.len() > 63 {
            Err(anyhow!("Generated filename is too long: {}", s))
        } else {
            Ok(s)
        }
    }

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
            //////viz_group: fields[8].parse()?,
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
    
    /// Update terrain tile. A new terrain tile has been added, and needs to be added to the database.
    fn update_tile(&self, conn: &mut PooledConn, texture_index: Option<u8>, asset_type: &str) -> Result<(), Error> {
        //  Allowed types. Must match exactly.
        assert!(asset_type == "BaseTexture" || asset_type == "EmissiveTexture" || asset_type == "SculptTexture" || asset_type == "Mesh");
        assert!(if asset_type == "BaseTexture" || asset_type == "EmissiveTexture" { texture_index.is_some() } else { true });
        //  Insert tile, or update hash and uuid if exists. 
        const SQL_UPDATE_TILE: &str = r"INSERT INTO tile_assets
                (grid, region_loc_x, region_loc_y, region_size_x, region_size_y,
                impostor_lod, texture_index, asset_hash, asset_uuid,
                asset_name, asset_type,
                creation_time) 
            VALUES 
                (:grid, :region_loc_x, :region_loc_y, :region_size_x, :region_size_y,
                :impostor_lod, :texture_index, :asset_hash, :asset_uuid,
                :asset_name, :asset_type,
                NOW()) 
            ON DUPLICATE KEY UPDATE
                asset_hash = :asset_hash, asset_uuid = :asset_uuid, creation_time = NOW()";
        //  UNIQUE INDEX (grid, region_loc_x, region_loc_y, impostor_lod, viz_group, texture_index)
        let asset_upload = self;
        let params = params! {
            "grid" => asset_upload.grid.to_lowercase(),
            "asset_name" => asset_upload.asset_name.clone(),
            "asset_type" => asset_type,
            "region_loc_x" => asset_upload.region_loc[0],
            "region_loc_y" => asset_upload.region_loc[1],
            "region_size_x" => asset_upload.region_size[0],
            "region_size_y" => asset_upload.region_size[1],
            "impostor_lod" => asset_upload.impostor_lod,
            //////"viz_group" => asset_upload.viz_group,
            "texture_index" => texture_index,
            "asset_uuid" => asset_upload.asset_uuid.clone(),
            "asset_hash" => asset_upload.asset_hash.clone(),
        };
        log::debug!("SQL terrain tile update: {:?}", params);
        conn.exec_drop(SQL_UPDATE_TILE, params)?;
        log::debug!("SQL terrain tile update succeeded.");
        Ok(())
    }
    
    /// Update a tile. A new tile has been added, and needs to be added to the database.
    pub fn update_texture_tile(&self, conn: &mut PooledConn, texture_index: u8, asset_type: &str) -> Result<(), Error> {
        self.update_tile(conn, Some(texture_index), asset_type)
    }
    
    //  Look up region name.
    //  Returns name of region if exact match. Otherwise searches for
    //  some name in a larger area containing the region of interest.
    fn look_up_region_name(conn: &mut PooledConn, grid: &str, loc: [u32;2], size: [u32;2]) -> Result<Option<String>, Error> {
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
    fn get_faces_json(&mut self, conn: &mut PooledConn) -> Result<serde_json::Value, Error> {
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
    
    /// Update terrain tile. A new terrain tile has been added, and needs to be added to the database.
    /// ***WRONG*** for a mesh tile.
    pub fn update_mesh_tile(&mut self, conn: &mut PooledConn) -> Result<(), Error> {
        //  Most of the info we need is in asset_upload, but we also need:
        //  - name
        //  - face texture data.
        log::debug!("Update mesh tile: {:?}", self);
        todo!();    // no mesh tiles yet
/*
        let faces_json = self.get_faces_json(asset_upload)?;
        let name_opt = self.look_up_region_name(&asset_upload.grid.to_lowercase(), asset_upload.region_loc, asset_upload.region_size, )?;
        //  Name is only for debug and documentation
        let name = if let Some(name) = name_opt { name } else { "(UNKNOWN)".to_string() };
        //  Valid sculpt tile.  Update tile assets.
        self.update_tile(asset_upload, None, "SculptTexture")?;        
        let mesh_uuid = Some(asset_upload.asset_uuid.clone());
        let sculpt_uuid = None;
        asset_upload.update_impostor_info(self.conn, &name, mesh_uuid, sculpt_uuid, faces_json)
*/
    }

    /// Update a sculpt tile.
    pub fn update_sculpt_tile(&mut self, conn: &mut PooledConn) -> Result<(), Error> {
        //  Most of the info we need is in asset_upload, but we also need:
        //  - name
        //  - face texture data.
        log::debug!("Update sculpt tile: {:?}", self);
        let faces_json = self.get_faces_json(conn)?;
        let name_opt = AssetUpload::look_up_region_name(conn, &self.grid.to_lowercase(), self.region_loc, self.region_size, )?;
        //  Name is only for debug and documentation
        let name = if let Some(name) = name_opt { name } else { "(UNKNOWN)".to_string() };
        //  Valid sculpt tile.  Update tile assets.
        self.update_tile(conn, None, "SculptTexture")
/*     
        let sculpt_uuid = Some(asset_upload.asset_uuid.clone());
        let mesh_uuid = None;
        self.update_impostor_info(conn, &name, mesh_uuid, sculpt_uuid, faces_json)
*/
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
