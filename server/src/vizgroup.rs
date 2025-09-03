//! vizgroup.rs - compute the visibility groups of regions.
//!
//! The visibiilty groups indicate which regions can be seen
//! from which other regions. If there is a path of connected
//! regions from one region to another, the regions can see
//! each other. 
//!
//! It's a transitive closure on "adjacent".
//! 
//! Corners are adjacent on Open Simulator but not Second Life.
//!
//! Animats
//! September, 2025
//! License: LGPL.
//!
use mysql::{OptsBuilder, Opts, Conn, Pool};
use serde::{Deserialize};
use mysql::{PooledConn, params};
use mysql::prelude::{Queryable, AsStatement};
use anyhow::{Error, anyhow};

/// RegionData - info about one region relevant to this computation.
#[derive(Debug, Clone)]
pub struct RegionData {
    /// Which grid
    grid: String,
    /// X
    region_coords_x: u32,
    /// Y
    region_coords_y: u32,
    /// X size
    size_x: u32,
    /// Y size
    size_y: u32,
    /// Region name
    name: String,
}

/// Vizgroups - find all the visibility groups
pub struct VizGroups {
}

impl VizGroups {
    /// Usual new
    pub fn new() -> Self {
        Self {
        }
    }
    
    /// Build from database
    pub fn build(&mut self, conn: &mut PooledConn) -> Result<(), Error> {
        println!("Build start");    // ***TEMP***
        const SQL_SELECT: &str = r"SELECT grid, region_coords_x, region_coords_y, size_x, size_y, name FROM raw_terrain_heights ORDER BY grid, region_coords_x, region_coords_y";
        
        let _all_regions = conn
            .query_map(
                SQL_SELECT,
                |(grid, region_coords_x, region_coords_y, size_x, size_y, name)| {
                    let region_data = RegionData { grid, region_coords_x, region_coords_y, size_x, size_y, name };
                    println!("{:?}", region_data);  // ***TEMP***                       
                },
        )?;
        Ok(())
    }
}
