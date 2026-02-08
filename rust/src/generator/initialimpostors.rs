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
//!     License: LGPL.
//!     Animats
//!     February, 2026.
//
use anyhow::{Error, anyhow};
use mysql::{PooledConn, params};
use mysql::prelude::Queryable;
use uuid::{Uuid};
use crate::{RegionData};
use common::{RegionImpostorData, RegionImpostorFaceData, HeightField};
//////use mysql::prelude::{Queryable};

/// The initial impostors.
pub struct InitialImpostors {
    /// SQL connection
    conn: PooledConn,
}

impl InitialImpostors {
    /// Usual new
    pub fn new(conn: PooledConn) -> Result<Self, Error> {

        //  ***CLEAR initial_region_impostors table***
        Ok(Self {
            conn
        })
    }
    
    /// Add one impostor (sculpt or mesh) to the table. UUIDs may be null.
    /// This is a pure insert into a table that starts empty. Duplicates should not happen.
    pub fn add_impostor(&mut self, region_impostor_data: RegionImpostorData) -> Result<(), Error> {
        log::debug!("Inserting {:?} into initial_impostors.", region_impostor_data.name);
        //  We have all the info now. Update the region_impostor table.
        //  Insert tile, or update hash and uuid if exists. 
        const SQL_IMPOSTOR: &str = r"INSERT INTO initial_impostors
                (grid, name, region_loc_x, region_loc_y, region_size_x, region_size_y, viz_group,
                scale_x, scale_y, scale_z, 
                elevation_offset, impostor_lod, viz_group, 
                mesh_uuid, sculpt_uuid,
                mesh_hash, sculpt_hasn
                water_height, creation_time, faces_json) 
            VALUES 
                (:grid, :name, :region_loc_x, :region_loc_y, :region_size_x, :region_size_y, :viz_group,
                :scale_x, :scale_y, :scale_z,
                :elevation_offset, :impostor_lod, :viz_group, 
                :mesh_uuid, :sculpt_uuid, 
                :mesh_hash, :sculpt_hash,
                :water_height, NOW(), :faces_json)";
               
        let insert_params = params! {
                "grid" => region_impostor_data.grid.to_lowercase().clone(),
                "name" => region_impostor_data.name,
                "mesh_uuid" => region_impostor_data.mesh_uuid,
                "sculpt_uuid" => region_impostor_data.sculpt_uuid,
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
        Ok(self.conn.exec_drop(SQL_IMPOSTOR, insert_params)?)
    }
}

/// Type of tile
pub enum TileType {
    /// As a sculpt
    Sculpt,
    /// As a mesh
    Mesh
}

/// Format conversion.
//  There's too much conversion between similar formats in this program.
//  Some of that is from having to put coordinates into SQL columns.
//  SQL has neither tuples nor arrays.
pub fn assemble_region_impostor_data(tile_type: TileType, region: &RegionData, height_field: HeightField, viz_group: u32, asset_hash: &str, asset_uuid_opt: Option<Uuid>, face_data: &[RegionImpostorFaceData], terrain_hash: &str, terrain_uuid: Option<Uuid>) -> RegionImpostorData {
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
