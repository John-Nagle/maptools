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
use anyhow::{Error};
use mysql::{PooledConn, params};
use mysql::prelude::Queryable;
use uuid::{Uuid};
use json::parse;
use crate::{RegionData};
use crate::{RegionImpostorData, RegionImpostorFaceData, HeightField};
use crate::{uuid_opt_to_string};
//////use mysql::prelude::{Queryable};

/// Type of tile
pub enum TileType {
    /// As a sculpt
    Sculpt,
    /// As a mesh
    Mesh
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
    pub fn find_missing_uuids(conn: &mut PooledConn, grid: &str) -> Result<Vec<RegionData>, Error> {
        const SQL_SELECT_MISSING_TILE: &str = r"SELECT region_loc_x, region_loc_y, name, region_size_x, region_size_y,
            mesh_hash, mesh_uuid, sculpt_hash, sculpt_uuid,
            faces_json
            FROM initial_impostors             
            WHERE (grid = :grid) AND (
                (mesh_hash IS NOT NULL AND mesh_uuid IS NULL) 
                OR (sculpt_hash IS NOT NULL AND sculpt_uuid IS NULL)
                )
            LIMIT 20";
            
        /*
                        OR (sculpt_hash IS NOT NULL AND sculpt_uuid IS NULL)
                OR EXISTS (
                    SELECT 1 FROM jsonb_array_elements(faces_json) AS elem
                    WHERE elem -> base_texture_uuid IS NULL
                    )
                )
        */
        let select_params = params! {
            "grid" => grid.to_lowercase()
        }; 
        //  Check sculpt/mesh IDs.
        let mut tiles_missing_uuids = conn.exec_map(
            SQL_SELECT_MISSING_TILE,
            &select_params, 
            |(region_loc_x, region_loc_y, name, region_size_x, region_size_y,
            mesh_hash, mesh_uuid, sculpt_hash, sculpt_uuid, impostor_lod,
            faces_json):
            (u32, u32, String, u32, u32,
            String, String, String, String, u8,
            String) | {
                let region_data = RegionData {
                    grid: grid.to_string(),
                    region_loc_x,
                    region_loc_y,
                    region_size_x,
                    region_size_y,
                    name,
                    lod: impostor_lod,
                    };
                log::debug!("Missing sculpt UUID for {:?}", region_data);
                region_data
            })?;
        //  Check texture IDs, which is a full slow table scan.
        //  We can't get MySQL 8.0 to do this for us.
        const SQL_SELECT_MISSING_TEXTURE: &str = r"SELECT region_loc_x, region_loc_y, name, region_size_x, region_size_y, impostor_lod,
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
            |(region_loc_x, region_loc_y, name, region_size_x, region_size_y, impostor_lod,        
            faces_json):
            (u32, u32, String, u32, u32, u8,
            String) | {
                //  Keep ones where there is a problem.
                let face_data_result: Result<Vec<RegionImpostorFaceData>, _> = serde_json::from_str(&faces_json);
                let keep = match face_data_result {
                    Ok(v) => v.iter().find(|face: &&RegionImpostorFaceData| is_missing_uuid(*face)).is_some(),
                    Err(e) => true
                };
                if keep { 
                    //  Bad entry, keep.                
                    let region_data = RegionData {
                        grid: grid.to_string(),
                        region_loc_x,
                        region_loc_y,
                        region_size_x,
                        region_size_y,
                        name,
                        lod: impostor_lod,
                        };
                    log::debug!("Missing texture UUID for {:?}", region_data);
                    tiles_missing_texture_uuids.push(region_data);
                }
                ()
            })?;
        tiles_missing_texture_uuids.append(&mut tiles_missing_uuids);
        Ok(tiles_missing_texture_uuids)
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
