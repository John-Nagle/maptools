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
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug)]
pub struct LiveBlock {
    /// This block
    region_data: RegionData,
    /// Link to VizGroup
    viz_group: Rc<RefCell<VizGroup>>,
}

impl LiveBlock {
    /// Usual new
    pub fn new(region_data: &RegionData, completed_groups_weak: &Weak<RefCell<CompletedGroups>>) -> Self {
        Self {
            region_data: region_data.clone(),
            viz_group: VizGroup::new(region_data.clone(), completed_groups_weak),
        }
    }
    
    /// Merge the VizGroups of two LiveBlock items.
    /// Both LiveBlocks get an Rc to the same VisGroup.
    pub fn merge(&mut self, other: &mut LiveBlock) {
        println!("Merging"); // ***TEMP***
        if self.viz_group != other.viz_group {
            self.viz_group.borrow_mut().merge(&mut other.viz_group.borrow_mut());
            other.viz_group = self.viz_group.clone()
        }
    }
    
    /// y-adjacent - true if adjacent in y.
    /// Called while iterating over a single column.
    fn y_adjacent(&self, b: &mut LiveBlock, tolerance: u32) -> bool {
        assert!(self.region_data.region_coords_y <= b.region_data.region_coords_y); // ordered properly, a < b in Y
        self.region_data.region_coords_y + self.region_data.size_y + tolerance >= b.region_data.region_coords_y
    }
    
    /// xy-adjacent - true if adjacent in x and y, on different columns.
    /// Called when iterating over two columns in sync.
    fn xy_adjacent(&self, b: &mut LiveBlock, tolerance: u32) -> bool {
        assert!(self.region_data.region_coords_x + self.region_data.size_x <= b.region_data.region_coords_x); // columns must be adjacent in X.
        //  True if overlaps in Y.
        // ***MORE***
        let ax0 = self.region_data.region_coords_y;
        let ax1 = ax0 + self.region_data.size_y;
        let bx0 = b.region_data.region_coords_y;
        let bx1 = bx0 + b.region_data.size_y;
        let overlap = ax0 <= bx1 && ax1 >= bx0;
        println!("XY-adjacent test: overlap: {}\n
            {:?}\nvs {:?}", overlap, self, b); // ***TEMP***
        overlap
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
    /// Usual new
    pub fn new() -> Self {
        Self {
            live_blocks: BTreeMap::new(),
        }
    }

    /// Purge all blocks whose X edge is below or equal to the limit.
    /// This is all of them on SL, but larger regions on OS might be kept.
    fn purge_below_x_limit(&mut self, x_limit: u32) {
        self.live_blocks.retain(|_, v| v.region_data.region_coords_x + v.region_data.size_x > x_limit);
    }
}

/// A set of regions which all have the same viz group
#[derive(Debug)]
pub struct VizGroup {
    /// All in this viz group are in this string.
    pub grid: String,
    /// Regions
    /// Will probably change to a different data structure
    /// This is inside an option so we can take it later.
    pub regions: Option<Vec<RegionData>>,
    /// Backlink to completed groups so they can be updated from drop.
    completed_groups_weak: Weak<RefCell<CompletedGroups>>,
}

impl PartialEq for VizGroup {
    /// Equality test.
    //  ***TOO EXPENSIVE - scans entire region list.***
    //  ***add a serial number or something.
    fn eq(&self, other: &Self) -> bool {
        self.grid == other.grid
        && self.regions == other.regions
    }
}

impl Drop for VizGroup {

    /// Drop happens when no live block is using this VizGroup.
    /// Thus, that VizGroup is complete.
    /// The group is delivered to VizGroups as done.
    fn drop(&mut self) {
        let mut completed_groups = self.completed_groups_weak.upgrade().expect("Unable to upgrade vizgroups");
        if let Some(group) = self.regions.take() {
            completed_groups.borrow_mut().push(group);
        }
    }
}

impl VizGroup {

    /// New, with the first region, and a back link to the VizGroups
    pub fn new(region: RegionData, completed_groups_weak: &Weak<RefCell<CompletedGroups>>) -> Rc<RefCell<Self>> {
        let new_item = Self {
            grid: region.grid.clone(),
            regions: Some(vec![region]),
            completed_groups_weak: completed_groups_weak.clone()
        };
        Rc::new(RefCell::new(new_item))
    }
    /// Merge another VizGroup into this one. The other group cannot be used again.
    pub fn merge(&mut self, other: &mut VizGroup) {
        assert_eq!(self.grid, other.grid);
        self.regions.as_mut().expect("Regions should not be None").append(&mut other.regions.take().expect("Regions should not be none"));
    }
}

type CompletedGroups = Vec<Vec<RegionData>>;

/// Vizgroups - find all the visibility groups
pub struct VizGroups {
    /// The active column
    column: Vec<LiveBlock>,
    /// Previous region data while inputting a column
    prev_region_data: Option<RegionData>,
    /// Live blocks. The blocks that touch or pass the current column.
    /// Ordered by Y.
    live_blocks: LiveBlocks,
    /// Completed groups. This is the output from transitive closure.
    /// No ordering
    completed_groups: Rc<RefCell<CompletedGroups>>,
    /// Tolerance. 0 or 1. 1 expands regions 1 unit for the overlap test.
    /// This makes corner adjacency work for Open Simulator
    tolerance: u32,
}

impl VizGroups {
    /// Usual new
    pub fn new() -> Self {
        Self {
            column: Vec::new(),
            prev_region_data: None,
            completed_groups: Rc::new(RefCell::new(Vec::new())),
            live_blocks: LiveBlocks::new(),
            tolerance: 0,
        }
    }

    /// Check the current and previous live block lists.
    /// They're both sequential in y.
    fn check_overlap_live_block_columns(&mut self) {
        //  Create iterators for existing live blocks and new column.
        let mut prev_iter = self.live_blocks.live_blocks.iter_mut();
        let mut prev_opt = prev_iter.next();
        let mut curr_iter = self.column.iter_mut();
        let mut curr_opt = curr_iter.next();
        loop {
            if let Some(ref mut prev) = prev_opt {
                if let Some(ref mut curr) = curr_opt {
                    //  Test if we want to merge viz groups
                    if prev.1.xy_adjacent(curr, self.tolerance) {
                        prev.1.merge(curr)
                    }
                    
                    if curr.region_data.region_coords_y < prev.1.region_data.region_coords_y {
                        curr_opt = curr_iter.next();
                    } else {
                        prev_opt = prev_iter.next();
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
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
        //  If two live blocks in this list overlap, merge their viz groups.
        //  This is the check for overlap in Y.
        let mut prev_opt: Option<&mut LiveBlock> = None;
        for item in &mut self.column {
            if let Some(prev) = prev_opt {
                if prev.y_adjacent(item, self.tolerance) {
                    prev.merge(item)
                }
            }
            prev_opt = Some(item);
        }
        //  Next, need the check for overlap in X, between existing live blocks
        //  and new live blocks
        self.check_overlap_live_block_columns();
        
        //  ***MORE***
        //  Compare previous list of live blocks with this one. If there is
        //  overlap, merge their viz groups.
        //  ***MORE***
        //  Update the list of live blocks.
        //  Ones that ended at the column edge disappear.
        //  All new ones are added.
        println!("End column. {} regions.", self.column.len());
        if !self.column.is_empty() {
            //  Purge now-dead live blocks. This will be all of them on SL, but wide regions on OS may not be ready to die yet.
            let x_limit = self.column[0].region_data.region_coords_x;
            self.live_blocks.purge_below_x_limit(x_limit);
            //  Add new live blocks.
            //////self.column.iter().map(|b| self.live_blocks.live_blocks.insert(b.region_data.region_coords_y, b));
            //////let _  = self.column.drain(..).map(|b| self.live_blocks.live_blocks.insert(b.region_data.region_coords_y, b));
            //  ***Proper way above does nothing***
            
            while let Some(b) = self.column.pop() {
                self.live_blocks.live_blocks.insert(b.region_data.region_coords_y, b);
            }
            println!("{} live blocks", self.live_blocks.live_blocks.len()); // ***TEMP***
            assert!(self.column.is_empty());
        }
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
        self.column.push(LiveBlock::new(&region_data, &Rc::<RefCell<Vec<Vec<RegionData>>>>::downgrade(&self.completed_groups)));
        self.prev_region_data = Some(region_data);                  
    }

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

//  Unit test.
//  The test data represents this pattern.
//  Ordered by x, y.
//  Result should be three VizGroup items.
//
//  X    XXXXXX
//  X XX X    X
//  X    X X  X
//  X    X X  X
//  XXXXXX
//
#[test]
fn test_visgroup() {
    /// Test pattern
    /// Format: RegionData { grid, region_coords_x, region_coords_y, size_x, size_y, name }; 
    const TEST_PATTERN: [(&str,u32, u32, u32, u32, &str);24] = [
        ( "Test", 0, 0, 100, 100, "Bottom left" ),
        ( "Test", 0, 100, 100, 100, "Left 100" ),
        ( "Test", 0, 200, 100, 100, "Left 200" ),
        ( "Test", 0, 300, 100, 100, "Left 300" ),
        ( "Test", 0, 400, 100, 100, "Left 400" ),
        ( "Test", 100, 0, 100, 100, "Bottom 100" ),
        ( "Test", 200, 0, 100, 100, "Bottom 200" ),
        ( "Test", 200, 300, 100, 100, "Tiny West" ),
        ( "Test", 300, 0, 100, 100, "Bottom 300" ), 
        ( "Test", 300, 300, 100, 100, "Tiny East" ),
        ( "Test", 400, 0, 100, 100, "Bottom 400" ),
        ( "Test", 500, 0, 100, 100, "Bottom 500" ),
        ( "Test", 500, 100, 100, 100, "Column 5-1" ),
        ( "Test", 500, 200, 100, 100, "Column 5-2" ),
        ( "Test", 500, 300, 100, 100, "Column 5-3" ),
        ( "Test", 500, 400, 100, 100, "Column 5-4" ),
        ( "Test", 600, 400, 100, 100, "Top 600" ),
        ( "Test", 700, 100, 100, 200, "Tall skinny region" ),
        ( "Test", 700, 400, 100, 100, "Top 700" ),
        ( "Test", 800, 400, 100, 100, "Top 800" ),   
        ( "Test", 900, 100, 100, 100, "Right 100" ),
        ( "Test", 900, 200, 100, 100, "Right 200" ),
        ( "Test", 900, 300, 100, 100, "Right 300" ),
        ( "Test", 900, 400, 100, 100, "Right 400" )];
        
    let test_data: Vec<_> = TEST_PATTERN.iter().map(|(grid, region_coords_x, region_coords_y, size_x, size_y, name)| 
        RegionData { grid: grid.to_string(), region_coords_x: *region_coords_x, region_coords_y: *region_coords_y, 
        size_x: *size_x, size_y: *size_y, name: name.to_string() }).collect(); 
        
    let mut viz_groups = VizGroups::new();
    for item in test_data {
        viz_groups.add_region_data(item);
    }
    viz_groups.end_grid();
    //  Display results
    println!("Result: Viz groups: {:?}", viz_groups.completed_groups.borrow());               
}
