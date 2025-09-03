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
    
    fn end_column(&mut self, column: &[RegionData] ) {
        println!("End column.");
    }
    
    fn end_grid(&mut self) {
        println!("End grid.");
    }
    
    /// Build from database
    pub fn build(&mut self, conn: &mut PooledConn) -> Result<(), Error> {
        println!("Build start");    // ***TEMP***
        //  The loop here is sequential data processing with control breaks when a field changes.
        const SQL_SELECT: &str = r"SELECT grid, region_coords_x, region_coords_y, size_x, size_y, name FROM raw_terrain_heights ORDER BY grid, region_coords_x, region_coords_y";
        let mut prev_region_data: Option<RegionData> = None;
        let mut column = Vec::new();
        let _all_regions = conn
            .query_map(
                SQL_SELECT,
                |(grid, region_coords_x, region_coords_y, size_x, size_y, name)| {
                    let region_data = RegionData { grid, region_coords_x, region_coords_y, size_x, size_y, name };
                    println!("{:?}", region_data);  // ***TEMP*** 
                    if let Some(prev) = &prev_region_data {
                        if region_data.grid != prev.grid {
                            self.end_column(&column);
                            column.clear();
                            column.push(region_data.clone());
                            self.end_grid();
                        } else if region_data.region_coords_x != prev.region_coords_x {
                            self.end_column(&column);
                            column.clear();
                            column.push(region_data.clone());
                        }
                    };    
                    prev_region_data = Some(region_data);                  
                },	
        )?;
        self.end_column(&column);
        column.clear();
        self.end_grid();
        Ok(())
    }
}
