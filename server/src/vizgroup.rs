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

/// A rectangle of interest which might touch a object in an incoming column.
pub struct LiveBlock {
}

/// An ordered sequence of LiveBlock items hwich might touch an object in an incoming column.
/// When a new column comes in, any LiveBlock which doesn't reach that far is purged.
//  Needs an ordered representation.
struct LiveBlocks {
    // ***MORE***
}

impl LiveBlocks {
    /// Test for overlap between a column and the live blocks.
    //  *** What should this return? ***
    fn test_overlap(&self, column: &[RegionData]) -> usize {
        todo!();
    } 
    /// Purge all blocks whose x edge is below the limit.
    /// This is all of them on SL, but larger regions on OS might be kept.
    fn purge_below_limit(lim: u32) -> Vec<LiveBlock> {
        todo!();
    }
    
    /// Add all the regions in a column to the live blocks.
    fn add_column(&mut self, column: &[RegionData]) {
        todo!();
    }
}

/// A set of regions which all have the same viz group
pub struct VizGroup {
    /// All in this viz group are in this string.
    pub grid: String,
    /// Regions
    /// Will probably change to a different data structure
    pub regions: Vec<RegionData>,
}

impl VizGroup {

    /// Merge two VizGroups, consuming them.
    pub fn merge(mut a: VizGroup, b: VizGroup) -> VizGroup {
  	    assert_eq!(a.grid, b.grid);
        a.regions.extend(b.regions);
        Self {
            grid: a.grid,
            regions: a.regions
        }
    }
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
        for region_data in column {
            println!("{:?}", region_data);  // ***TEMP*** 
        }
        println!("End column. {} regions.", column.len());
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
                    if let Some(prev) = &prev_region_data {
                        if region_data.grid != prev.grid {
                            self.end_column(&column);
                            column.clear();
                            self.end_grid();
                        } else if region_data.region_coords_x != prev.region_coords_x {
                            self.end_column(&column);
                            column.clear();
                        }
                    };
                    //  Add to column, or start new column.
                    column.push(region_data.clone());
                    prev_region_data = Some(region_data);                  
                },	
        )?;
        self.end_column(&column);
        column.clear();
        self.end_grid();
        Ok(())
    }
}
