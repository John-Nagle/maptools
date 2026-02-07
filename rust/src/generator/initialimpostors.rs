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
use mysql::{PooledConn};
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
    pub fn add_impostor(&mut self, region_impostor_data: RegionImpostorData) -> Result<(), Error> {
        todo!();
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
