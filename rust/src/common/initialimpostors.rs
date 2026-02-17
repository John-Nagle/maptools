//! initialimpostors.rs -- generate the initial_impostor_regions table.
//!
//! The initial_impostor_regions table is the basis for the final impostor_regions
//! table. It has everything except the UUIDs of assets which still need to be created.
//! It's created here, and uploadterrain updates it with new assets.
//! When all UUIDs are non-null, the impostor_regions info is complete, and
//! this table is copied over to the impostor_regions table as an atomic operation.
//!
//! Part of the Animats impostor system
//!
//!
//! License: LGPL.
//! Animats
//! February, 2026.
//
use anyhow::{Error, anyhow};
use mysql::{PooledConn, params};
use mysql::prelude::Queryable;
use uuid::{Uuid};
use crate::{RegionData};
use crate::{RegionImpostorData, RegionImpostorFaceData, HeightField};
use crate::{uuid_opt_to_string};

/// Type of tile
pub enum TileType {
    /// As a sculpt
    Sculpt,
    /// As a mesh
    Mesh
}

/// These values uniquely identify an impostor record.
/// The initial impostors table has UNIQUE KEY (grid, region_loc_x, region_loc_y, impostor_lod, viz_group)
#[derive(Debug, Clone)]
pub struct UniqueImpostorKey {
    /// Which grid
    grid: String,
    /// Region location X
    region_loc_x: u32,
    /// Region location Y
    region_loc_y: u32,
    /// Impostor LOD. We don't need size if we have LOD.
    impostor_lod: u8,
    /// Viz group. The same multi-region tile can be different in different viz groups.
    viz_group: u32,
}

/// The initial impostors.
pub struct InitialImpostors {
}

impl InitialImpostors {
    /// Usual new
    pub fn new() -> Self {
        Self {
        }
    }
    
    /// Add one impostor (sculpt or mesh) to the table. UUIDs may be null.
    /// This is a pure insert into a table that starts empty. Duplicates should not happen.
    pub fn add_impostor(conn: &mut PooledConn, region_impostor_data: RegionImpostorData) -> Result<(), Error> {
        log::debug!("Inserting {:?} into initial_impostors.", region_impostor_data.name);
        //  We have all the info now. Update the region_impostor table.
        //  Insert tile, or update hash and uuid if exists. 
        const SQL_IMPOSTOR: &str = r"INSERT INTO initial_impostors
                (grid, name, region_loc_x, region_loc_y, region_size_x, region_size_y,
                scale_x, scale_y, scale_z, 
                elevation_offset, impostor_lod, viz_group,
                mesh_uuid, sculpt_uuid,
                mesh_hash, sculpt_hash,
                water_height, creation_time, faces_json) 
            VALUES 
                (:grid, :name, :region_loc_x, :region_loc_y, :region_size_x, :region_size_y,
                :scale_x, :scale_y, :scale_z,
                :elevation_offset, :impostor_lod, :viz_group, 
                :mesh_uuid, :sculpt_uuid, 
                :mesh_hash, :sculpt_hash,
                :water_height, NOW(), :faces_json)";
               
        let insert_params = params! {
                "grid" => region_impostor_data.grid.to_lowercase().clone(),
                "name" => region_impostor_data.name,
                "mesh_uuid" => uuid_opt_to_string(region_impostor_data.mesh_uuid),
                "sculpt_uuid" => uuid_opt_to_string(region_impostor_data.sculpt_uuid),
                "mesh_hash" => region_impostor_data.mesh_hash,
                "sculpt_hash" => region_impostor_data.sculpt_hash,
                "region_loc_x" => region_impostor_data.region_loc[0],
                "region_loc_y" => region_impostor_data.region_loc[1],
                "region_size_x" => region_impostor_data.region_size[0],
                "region_size_y" => region_impostor_data.region_size[1],
                "scale_x" => region_impostor_data.scale[0], // ***CONVERT TO INT***
                "scale_y" => region_impostor_data.scale[1], // ***CONVERT TO INT***
                "scale_z" => region_impostor_data.scale[2],
                "impostor_lod" => region_impostor_data.impostor_lod,
                "viz_group" => region_impostor_data.viz_group,
                "elevation_offset" => region_impostor_data.elevation_offset,
                "water_height" => region_impostor_data.water_height,
                "faces_json" => serde_json::to_string(&region_impostor_data.faces)?,
            };
        //  Finally insert into the impostor table
        log::debug!("Inserting impostor into initial_impostors, params: {:?}", insert_params);
        Ok(conn.exec_drop(SQL_IMPOSTOR, insert_params)?)
    }
    
    /// Truncate the table for one grid This table is re-created on each run of generateterrain.
    pub fn clear_grid(conn: &mut PooledConn, grid: &str) -> Result<(), Error> {
        const SQL_DELETE: &str = r"DELETE FROM initial_impostors WHERE grid = :grid;";
        let delete_params = params! {
            "grid" => grid.to_lowercase()
        };
        Ok(conn.exec_drop(SQL_DELETE, delete_params)?)
    }
    
    /// Find missing UUIDs. When there are none, intitial_impostors is in sync and can be deployed as region_impostors.
    pub fn find_missing_uuids(conn: &mut PooledConn, grid: &str) -> Result<Vec<UniqueImpostorKey>, Error> {
        const SQL_SELECT_MISSING_TILE: &str = r"SELECT region_loc_x, region_loc_y, name, region_size_x, region_size_y,
            mesh_hash, mesh_uuid, sculpt_hash, sculpt_uuid,
            faces_json
            FROM initial_impostors             
            WHERE (grid = :grid) AND (
                (mesh_hash IS NOT NULL AND mesh_uuid IS NULL) 
                OR (sculpt_hash IS NOT NULL AND sculpt_uuid IS NULL)
                )
            LIMIT 20";
        let select_params = params! {
            "grid" => grid.to_lowercase()
        }; 
        //  Check sculpt/mesh IDs.
        let mut tiles_missing_uuids = conn.exec_map(
            SQL_SELECT_MISSING_TILE,
            &select_params, 
            |(region_loc_x, region_loc_y, name, 
            mesh_hash, mesh_uuid, sculpt_hash, sculpt_uuid, impostor_lod, viz_group,
            faces_json):
            (u32, u32, String,
            String, String, String, String, u8, u32,
            String) | {
                let tile_key = UniqueImpostorKey {
                    grid: grid.to_string(),
                    region_loc_x,
                    region_loc_y,
                    impostor_lod,
                    viz_group,
                    };
                log::debug!("Missing sculpt UUID for {:?}   Sculpt hash: {}, sculpt uuid {:?}", tile_key, sculpt_hash, sculpt_uuid);
                tile_key
            })?;
        //  Check texture IDs, which is a full slow table scan.
        //  We can't get MySQL 8.0 to do this for us.
        const SQL_SELECT_MISSING_TEXTURE: &str = r"SELECT region_loc_x, region_loc_y, name, impostor_lod, viz_group,
            faces_json
            FROM initial_impostors
            WHERE (grid = :grid)";
        let mut tiles_missing_texture_uuids = Vec::new();
        let is_missing_uuid = | v: &RegionImpostorFaceData | {
            v.base_texture_uuid.is_none() || (v.emissive_texture_hash.is_some() && v.emissive_texture_uuid.is_none())
        };
        let _ = conn.exec_map(
            SQL_SELECT_MISSING_TEXTURE,
            &select_params, 
            |(region_loc_x, region_loc_y, _name, impostor_lod, viz_group,     
            faces_json):
            (u32, u32, String, u8, u32,
            String) | {
                //  Keep ones where there is a problem.
                let face_data_result: Result<Vec<RegionImpostorFaceData>, _> = serde_json::from_str(&faces_json);
                let keep = match &face_data_result {
                    Ok(v) => v.iter().find(|face: &&RegionImpostorFaceData| is_missing_uuid(*face)).is_some(),
                    Err(_e) => true
                };
                if keep { 
                    //  Bad entry, keep.                
                    let tile_key = UniqueImpostorKey {
                        grid: grid.to_string(),
                        region_loc_x,
                        region_loc_y,
                        impostor_lod,
                        viz_group,
                        };
                    log::debug!("Missing texture UUID for {:?}, face_data: {:?}", tile_key, face_data_result);
                    tiles_missing_texture_uuids.push(tile_key);
                }
                ()
            })?;
        //  Perform repairs here.
        if !tiles_missing_texture_uuids.is_empty() {
            log::info!("{} tiles are missing texture UUIDs. Starting repair.", tiles_missing_texture_uuids.len());
            let tiles_still_missing_texture_uuids = Self::fix_missing_texture_uuids(conn, &tiles_missing_texture_uuids)?;
            if !tiles_still_missing_texture_uuids.is_empty() {
                log::error!("{} tiles are still missing texture UUIDs. Failed repair.", tiles_still_missing_texture_uuids.len());
                return Err(anyhow!("{} tiles are still missing texture UUIDs. Failed repair.", tiles_still_missing_texture_uuids.len()));
            } else {
                log::info!("Tile missing UUID repair successful.");
            }          
        }
        //  Construct a vec of all tiles with problems.
        tiles_missing_uuids.append(&mut tiles_missing_texture_uuids);
        Ok(tiles_missing_uuids)
    }
       
    /// Fix missing UUIDs. When there are none, initial_impostors is in sync and can be deployed as region_impostors.
    /// The tile list here must contain all the info in the UNIQUE INDEX (grid, region_loc_x, region_loc_y, impostor_lod, viz_group)
    /// so that the SELECT and UPDATE will match the same record. So UniqueImpostorKey is used.
    ///
    /// This is slow but is only applied to missing tiles.
    fn fix_missing_texture_uuids(conn: &mut PooledConn, keys: &[UniqueImpostorKey]) -> Result<Vec<UniqueImpostorKey>, Error> {
        let mut fails = Vec::new();
        for key in keys {
            if !Self::fix_missing_texture_uuids_for_tile(conn, key)? {
                fails.push((*key).clone())
            }
        }
        //  Return unsuccessful fixes. 
        Ok(fails)
    }
    
    /// Find and fix a missing UUID in a face texture entry.
    /// This tends to happen if something went wrong in upload and an upload had to be rerun.
    fn fix_missing_texture_uuids_for_tile(conn: &mut PooledConn, key: &UniqueImpostorKey) ->
            Result<bool, Error> {
        //  Get old json_faces.
        const SQL_SELECT_FACES_JSON: &str = r"SELECT faces_json FROM initial_impostors 
            WHERE grid = :grid
                AND region_loc_x = :region_loc_x 
                AND region_loc_y = :region_loc_y
                AND impostor_lod = :impostor_lod
                AND viz_group = :viz_group";
                
        const SQL_UPDATE_FACES_JSON: &str = r"UPDATE initial_impostors 
            SET faces_json = :faces_json
            WHERE grid = :grid
                AND region_loc_x = :region_loc_x 
                AND region_loc_y = :region_loc_y
                AND impostor_lod = :impostor_lod
                AND viz_group = :viz_group";
                               
        let key_params = params! {
            "grid" => key.grid.clone(),
            "region_loc_x" => key.region_loc_x,
            "region_loc_y" => key.region_loc_y,
            "impostor_lod" => key.impostor_lod,
            "viz_group" => key.viz_group,
        };
        //  Get entry to fix.
        let faces_json_opt: Option<String> = conn.exec_first(SQL_SELECT_FACES_JSON, key_params)?;
        if let Some(faces_json) = faces_json_opt {              
            let mut changed = false;
            let mut face_data: Vec<RegionImpostorFaceData> = serde_json::from_str(&faces_json)?;
            log::debug!("Faces before change: {:?}", face_data); // ***TEMP***
            for (face_id, face) in &mut face_data.iter_mut().enumerate() {
                if let Some(new_face) = Self::fix_missing_texture_uuid_for_face(conn, key, &face, face_id)? {
                    *face = new_face;
                    changed = true;
                }
            }
            log::debug!("Faces after change: {:?}", face_data); // ***TEMP***
            let faces_json: String = faces_json;
            //  Parse into a JSON string.
            let new_faces_json = serde_json::to_string(&face_data)?;
            if changed {
                let key_params = params! {
                    "grid" => key.grid.clone(),
                    "region_loc_x" => key.region_loc_x,
                    "region_loc_y" => key.region_loc_y,
                    "impostor_lod" => key.impostor_lod,
                    "viz_group" => key.viz_group,
                    "faces_json" => new_faces_json.to_string(),
                };
                log::info!("Fixed missing texture UUID: {:?}", key_params);    
                conn.exec_drop(SQL_UPDATE_FACES_JSON, key_params)?;        
            } else {
                log::warn!("Fixing missing texture UUIDs, no change to tile {:?}: {:?}", key, faces_json);
                return Ok(false)
            }
        } else {
            log::warn!("Fixing missing texture UUIDs, could not find tile {:?}", key);
            return Ok(false)
        }
        Ok(true)
    }
    
    /// Find and fix a missing UUID in a face texture entry.
    /// This tends to happen if something went wrong in upload and an upload had to be rerun.
    fn fix_missing_texture_uuid_for_face(conn: &mut PooledConn, key: &UniqueImpostorKey, face: &RegionImpostorFaceData, face_id: usize) 
            -> Result<Option<RegionImpostorFaceData>, Error> {
        let mut changed = false;
        let mut face = face.clone();
        //  Fix up base texture.
        if face.base_texture_uuid.is_none() {
            if let Some(uuid) = Self::look_up_uuid(conn, key, face_id, &face.base_texture_hash, "BaseTexture")? {
                face.base_texture_uuid = Some(uuid);
                changed = true;
            }
        }
        //  Fix up emissive texture if present.
        if let Some(hash) = &face.emissive_texture_hash {
            if face.emissive_texture_uuid.is_none() {
                if let Some(uuid) = Self::look_up_uuid(conn, key, face_id, hash, "EmissiveTexture")? {
                    face.emissive_texture_uuid = Some(uuid);
                    changed = true;
                }
            }
        };
        //  Do we have new face data?
        if changed {
            Ok(Some(face))
        } else {
            Ok(None)
        }
    }
    
    /// Look up a missing UUID in tile_assets.
    fn look_up_uuid(conn: &mut PooledConn, key: &UniqueImpostorKey, face_id: usize, asset_hash: &str, asset_type: &str) -> Result<Option<Uuid>, Error> {
        const SQL_LOOK_UP_UUID: &str = r"SELECT asset_uuid FROM tile_assets 
            WHERE  grid = :grid
                AND region_loc_x = :region_loc_x 
                AND region_loc_y = :region_loc_y
                AND impostor_lod = :impostor_lod
                AND asset_hash = :asset_hash
                AND asset_type = :asset_type";
        let select_params = params! {
            "grid" => key.grid.clone(),
            "region_loc_x" => key.region_loc_x,
            "region_loc_y" => key.region_loc_y,
            "impostor_lod" => key.impostor_lod,
            "asset_hash" => asset_hash,
            "asset_type" => asset_type,
        };
        //  Look up the tile. Hash is part of the key.
        log::debug!("Looking up tile UUID: {:?}", select_params);
        let uuid_opt: Option<String> = conn.exec_first(SQL_LOOK_UP_UUID, select_params)?;
        log::debug!("Looked up tile UUID: {:?}", uuid_opt);
        Ok(if let Some(uuid_str) = uuid_opt {
            Some(Uuid::parse_str(&uuid_str)?)
        } else {
            None
        })
    }
    
    /// Format conversion.
    //  There's too much conversion between similar formats in this program.
    //  Some of that is from having to put coordinates into SQL columns.
    //  SQL has neither tuples nor arrays.
    pub fn assemble_region_impostor_data(tile_type: TileType, region: &RegionData, height_field: &HeightField, viz_group: u32, 
        asset_hash: &str, asset_uuid_opt: Option<Uuid>, face_data: &[RegionImpostorFaceData]) -> RegionImpostorData {
        let (sculpt_hash, sculpt_uuid, mesh_hash, mesh_uuid) = match tile_type {
            TileType::Sculpt => (Some(asset_hash), asset_uuid_opt, None, None),
            TileType::Mesh => (None, None, Some(asset_hash), asset_uuid_opt)
        };
        //  This is valid but inefficient.
        let (scale, offset) = height_field.get_scale_offset().expect("Height field invalid, should be caught by caller.");
        RegionImpostorData {
            region_loc: [region.region_loc_x, region.region_loc_y],
            region_size: [region.region_size_x, region.region_size_y],     
            scale: [region.region_size_x as f32, region.region_size_y as f32, scale],
            impostor_lod: region.lod,
            viz_group,
            sculpt_uuid,   
            sculpt_hash: sculpt_hash.map(|s| s.to_string()),
            mesh_uuid,
            mesh_hash: mesh_hash.map(|s| s.to_string()),
            elevation_offset: offset,
            water_height: Some(height_field.water_level),
            name: Some(region.name.clone()),
            grid: region.grid.clone(),
            faces: face_data.into(),
        }
    }
}
