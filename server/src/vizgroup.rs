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
use std::cell::{RefCell};
use std::rc::{Rc, Weak};
use std::collections::{BTreeMap};

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

//  General concept of transitive closure algorithm.
//  (Tenative)
//
//  LiveBlocks have an Rc link to a VizGroup.
//  When we detect that a region in a new column touches
//  a LiveBlock or another region in the new column,
//  VizGroups are merged.
//  (This part is tricky for ownership reasons.)
//
//  When a LiveBlock is deleted, it drops its reference to the VizGroup.
//  When a VizGroup is deleted because no LiveBlock is referencing it,
//  that means a VizGroup is complete, so
//  ownership of the VizGroup's data is transferred to a vector of
//  completed VizGroup items in VizGroups.


/// A rectangle of interest which might touch a object in an incoming column.
pub struct LiveBlock {
    /// This block
    region_data: RegionData,
    /// Link to VizGroup
    viz_group: Rc<RefCell<VizGroup>>,
}

impl LiveBlock {
    /// Usual new
    pub fn new(region_data: &RegionData, viz_groups_weak: &Weak<RefCell<VizGroups>>) -> Self {
        Self {
            region_data: region_data.clone(),
            viz_group: VizGroup::new(region_data.clone(), viz_groups_weak),
        }
    }
}



/// An ordered sequence of LiveBlock items which might touch an object in an incoming column.
/// When a new column comes in, any LiveBlock which doesn't reach that far is purged.
//  Needs an ordered representation.
struct LiveBlocks {
    /// The blocks
    live_blocks: BTreeMap<u32, LiveBlock>,
}

impl LiveBlocks {
    /// Test for overlap between a column and the live blocks.
    //  *** What should this return? ***
    fn test_overlap(&self, column: &[RegionData]) -> usize {
        todo!();
    } 
    /// Purge all blocks whose X edge is below or equal to the limit.
    /// This is all of them on SL, but larger regions on OS might be kept.
    fn purge_below_x_limit(&mut self, x_limit: u32) {
        self.live_blocks.retain(|_, v| v.region_data.region_coords_x + v.region_data.size_x > x_limit);
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
    /// This is inside an option so we can take it later.
    pub regions: Option<Vec<RegionData>>,
    /// Backlink to VizGroups
    viz_groups_weak: Weak<RefCell<VizGroups>>,
}

impl Drop for VizGroup {

    /// Drop happens when no live block is using this VizGroup.
    /// Thus, that VizGroup is complete.
    /// The group is delivered to VizGroups as done.
    fn drop(&mut self) {
        let mut viz_groups = self.viz_groups_weak.upgrade().expect("Unable to upgrade vizgroups");
        viz_groups.borrow_mut().add_completed_group(self.regions.take().expect("Regions should not be None"));
    }
}

impl VizGroup {

    /// New, with the first region, and a back link to the VizGroups
    pub fn new(region: RegionData, viz_groups_weak: &Weak<RefCell<VizGroups>>) -> Rc<RefCell<Self>> {
        let new_item = Self {
            grid: region.grid.clone(),
            regions: Some(vec![region]),
            viz_groups_weak: viz_groups_weak.clone()
        };
        Rc::new(RefCell::new(new_item))
    }
    /// Merge another VizGroup into this one. The other group cannot be used again.
    pub fn merge(&mut self, other: &mut VizGroup) {
        assert_eq!(self.grid, other.grid);
        self.regions.as_mut().expect("Regions should not be None").append(&mut other.regions.take().expect("Regions should not be none"));
    }
/*
    /// Merge two VizGroups, consuming them.
    pub fn merge(mut a: VizGroup, b: VizGroup) -> VizGroup {
  	    assert_eq!(a.grid, b.grid);
        a.regions.extend(b.regions);
        Self {
            grid: a.grid,
            regions: a.regions
        }
    }
*/
}

/// Vizgroups - find all the visibility groups
pub struct VizGroups {
    /// The active column
    column: Vec<RegionData>,
    /// Previous region data while inputting a column
    prev_region_data: Option<RegionData>,
    /// Live blocks. The blocks that touch or pass the current column.
    /// Ordered by Y.
    live_blocks: Vec<LiveBlocks>,
    /// Completed groups. This is the output from transitive closure.
    /// No ordering
    completed_groups: Vec<Vec<RegionData>>,
}

impl VizGroups {
    /// Usual new
    pub fn new() -> Self {
        Self {
            column: Vec::new(),
            prev_region_data: None,
            completed_groups: Vec::new(),
            live_blocks: Vec::new(),
        }
    }
    
    /// Add a completed VizGroup. This is one connected area of regions.
    pub fn add_completed_group(&mut self, completed_group: Vec<RegionData>) {
        self.completed_groups.push(completed_group);
    }
    
    /// End of a column.
    /// Where all the real work gets done.
    /// Each entry in the new column has to be compared with the
    /// live blocks to check for overlap/touching, and with adjacent
    /// entries in the column to check for overlap/touching.
    /// Eacn new column entry creates a new VizGroup.
    /// Overlapped/touching groups get their VizGroups merged.
    /// ***WHAT HAPPENS FOR EMPTY COLUMN?***
    fn end_column(&mut self) {
        // ***MORE***
        for region_data in &self.column {
            println!("{:?}", region_data);  // ***TEMP*** 
        }
        //  Create a new list of live blocks from columns.
        //  Each live block gets its own VizGroup.
        //  If two live blocks in this list overlap, merge their viz groups.
        //  ***MORE***
        //  Compare previous list of live blocks with this one. If there is
        //  overlap, merge their viz groups.
        //  ***MORE***
        //  Update the list of live blocks.
        //  Ones that ended at the column edge disappear.
        //  All new ones are added.
        println!("End column. {} regions.", self.column.len());
        self.column.clear();
    }
    
    fn end_grid(&mut self) {
        self.end_column();
        println!("End grid.");
    }
    
    /// Add one item of region data.
    fn add_region_data(&mut self, region_data: RegionData) {
        if let Some(prev) = &self.prev_region_data {
            if region_data.grid != prev.grid {
                self.end_column();
                self.end_grid();
            } else if region_data.region_coords_x != prev.region_coords_x {
                self.end_column();
            }
        };
        //  Add to column, or start new column.
        self.column.push(region_data.clone());
        self.prev_region_data = Some(region_data);                  
    }
/*    
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
*/
    /// Build from database
    pub fn build(&mut self, conn: &mut PooledConn) -> Result<(), Error> {
        println!("Build start");    // ***TEMP***
        //  The loop here is sequential data processing with control breaks when a field changes.
        const SQL_SELECT: &str = r"SELECT grid, region_coords_x, region_coords_y, size_x, size_y, name FROM raw_terrain_heights ORDER BY grid, region_coords_x, region_coords_y";
        let _all_regions = conn
            .query_map(
                SQL_SELECT,
                |(grid, region_coords_x, region_coords_y, size_x, size_y, name)| {
                    let region_data = RegionData { grid, region_coords_x, region_coords_y, size_x, size_y, name }; 
                    self.add_region_data(region_data);                  
                },	
        )?;
        self.end_grid();
        Ok(())
    }
}
