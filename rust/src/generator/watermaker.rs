//! watermaker.rs
//!
//! Generation of Second Life terrain impostors for water-only regions.
//! This is mostly a dummy so we can do generation.
//!  
//! Animats, May, 2026
//! License: GPL

use common::{TileType, TerrainGeometry};
use anyhow::{Error};

#[derive(Debug)]
pub struct TerrainWater {
    /// Water level.
    water_height: f32,
}

impl TerrainWater {
    /// Usual new. All we have here is water height, because there is no terrain.
    pub fn new(water_height: f32) -> Self {
        Self {
            water_height
        }
    }
}

impl TerrainGeometry for TerrainWater {
    /// Get tile type for this variant.
    /// Not meaningful.
    fn get_tile_type(&self) -> TileType {
       TileType::Water
    }
    
    /// Get water height, which we put in here so we don't have to pass height field further down.
    fn get_water_height(&self) -> f32 {
        self.water_height
    }
    
    /// Get adjusted (with skirt) scale and offset.
    /// Not meaningful for water only tiles.
    fn get_adjusted_scale_offset(&self) -> Result<(f32, f32), Error> {
        Ok((1.0, 1.0))
    }
}
