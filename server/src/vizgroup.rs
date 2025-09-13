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
//! For Open Simulator, we add 1 to the size, to make corner contacts touch.
//!
//! Animats
//! September, 2025
//! License: LGPL.
//!
#![forbid(unsafe_code)]
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};

/// RegionData - info about one region relevant to this computation.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionData {
    /// Which grid
    pub grid: String,
    /// X
    pub region_coords_x: u32,
    /// Y
    pub region_coords_y: u32,
    /// X size
    pub size_x: u32,
    /// Y size
    pub size_y: u32,
    /// Region name
    pub name: String,
}

impl std::fmt::Display for RegionData {
    /// Just name and location, no size.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "\"{}\" ({}, {})",
            self.name, self.region_coords_x, self.region_coords_y
        )
    }
}

//  General concept of transitive closure algorithm.
//
//  LiveBlocks have an Rc link to a VizGroup.
//  When we detect that a region in a new column touches
//  a LiveBlock or another region in the new column,
//  VizGroups are merged.
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
    /// Weak link to self
    weak_link_to_self: WeakLiveBlockLink,
}

/// So we can have backpointers.
type LiveBlockLink = Rc<RefCell<LiveBlock>>;
/// Backpointers for LiveBlocks, so we can update them after merges.
type WeakLiveBlockLink = Weak<RefCell<LiveBlock>>;

impl LiveBlock {
    /// Usual new
    pub fn new(
        region_data: &RegionData,
        completed_groups_weak: &Weak<RefCell<CompletedGroups>>,
    ) -> Rc<RefCell<LiveBlock>> {
        Rc::new_cyclic(|weak_self| {
            RefCell::new(LiveBlock {
                region_data: region_data.clone(),
                viz_group: VizGroup::new(
                    region_data.clone(),
                    weak_self.clone(),
                    completed_groups_weak,
                ),
                weak_link_to_self: weak_self.clone(),
            })
        })
    }

    /// Two live blocks touch.
    /// Their VizGroup must be merged.
    /// And all live blocks that use the VizGroup that was merged out must be redirected to the new combined group.
    pub fn blocks_touch(&mut self, other: &LiveBlockLink) {
        //  Merge VizGroup data of the two LiveBlock items.
        //  At end, both share the same combined VizGroup, and the "other" VizGroup is dead, never to be used again.
        if !Rc::ptr_eq(&self.viz_group, &other.borrow().viz_group) {
            log::debug!(
                "Blocks with different viz groups touch: {} and {}",
                self.region_data,
                other.borrow().region_data
            );
            //  Merge the VizGroup sets.
            self.viz_group
                .borrow_mut()
                .merge(&mut other.borrow().viz_group.borrow_mut());

            //  Tell all other involved LiveBlock items about this merge.
            //  Cloning here clones a vector, but we have to get out from under those borrows.
            let self_shared_groups = self.viz_group.borrow().live_blocks_weak.clone();
            let other_shared_groups = other.borrow().viz_group.borrow().live_blocks_weak.clone();
            //  Now here we have to avoid a double mutable borrow. 
            //  So there's a test to check that we're not trying to borrow the LiveBlock we are working on.
            for weak_block in &self_shared_groups {
                if !Weak::ptr_eq(&self.weak_link_to_self, weak_block) {
                    if let Some(block) = weak_block.upgrade() {
                        block.borrow_mut().viz_group = self.viz_group.clone();
                    }
                }
            }
            for weak_block in &other_shared_groups {
                if !Weak::ptr_eq(&self.weak_link_to_self, weak_block) {
                    if let Some(block) = weak_block.upgrade() {
                        block.borrow_mut().viz_group = self.viz_group.clone();
                    }
                }
            }
        }
    }

    /// y-adjacent - true if adjacent in y.
    /// Called while iterating over a single column.
    fn y_adjacent(&self, bref: &LiveBlockLink, tolerance: u32) -> bool {
        let b = bref.borrow();
        assert!(self.region_data.region_coords_y <= b.region_data.region_coords_y); // ordered properly, a < b in Y
        self.region_data.region_coords_y + self.region_data.size_y + tolerance
            >= b.region_data.region_coords_y
    }

    /// xy-adjacent - true if adjacent in x and y, on different columns.
    /// Called when iterating over two columns in sync.
    fn xy_adjacent(&self, bref: &LiveBlockLink, tolerance: u32) -> bool {
        let b = bref.borrow();
        assert!(
            self.region_data.region_coords_x + self.region_data.size_x
                <= b.region_data.region_coords_x
        ); // columns must be adjacent in X.
        //  True if overlaps in Y.
        let a0 = self.region_data.region_coords_y;
        let a1 = a0 + self.region_data.size_y + tolerance;
        let b0 = b.region_data.region_coords_y;
        let b1 = b0 + b.region_data.size_y + tolerance;
        let overlap = a0 < b1 && a1 >= b0;
        log::trace!(
            "XY-adjacent test: overlap: ({}, {}) vs ({}, {}) overlap: {}",
            a0, a1, b0, b1, overlap
        );
        overlap
    }
}

/// An ordered sequence of LiveBlock items which might touch an object in an incoming column.
/// When a new column comes in, any LiveBlock which doesn't reach that far is purged.
//  Needs an ordered representation.
struct LiveBlocks {
    /// The blocks
    live_blocks: BTreeMap<u32, LiveBlockLink>,
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
        self.live_blocks.retain(|_, v| {
            let bk = v.borrow();
            bk.region_data.region_coords_x + bk.region_data.size_x > x_limit
        });
    }
}

/// A set of regions which all have the same viz group
#[derive(Debug)]
pub struct VizGroup {
    /// All in this viz group are in this string.
    pub grid: String,
    /// Regions
    /// Will probably change to a different data structure
    pub regions: Vec<RegionData>,
    /// Backlink to LiveBlocks that use this VizGroup.
    /// Used to tell the LiveBlock about a merge.
    pub live_blocks_weak: Vec<Weak<RefCell<LiveBlock>>>,
    /// Backlink to completed groups so they can be updated from drop.
    completed_groups_weak: Weak<RefCell<CompletedGroups>>,
}

impl Drop for VizGroup {
    /// Drop happens when no live block is using this VizGroup.
    /// Thus, that VizGroup is complete.
    /// The group is delivered to VizGroups as done.
    fn drop(&mut self) {
        let completed_groups = self
            .completed_groups_weak
            .upgrade()
            .expect("Unable to upgrade vizgroups");
        log::debug!("Drop of VizGroup: {} regions", self.regions.len());
        if !self.regions.is_empty() {
            completed_groups.borrow_mut().push(self.regions.clone());
        }
    }
}

impl VizGroup {
    /// New, with the first region, and a back link to the VizGroups
    pub fn new(
        region: RegionData,
        live_block_weak: WeakLiveBlockLink,
        completed_groups_weak: &Weak<RefCell<CompletedGroups>>,
    ) -> Rc<RefCell<Self>> {
        let new_item = Self {
            grid: region.grid.clone(),
            regions: vec![region],
            live_blocks_weak: vec![live_block_weak],
            completed_groups_weak: completed_groups_weak.clone(),
        };
        Rc::new(RefCell::new(new_item))
    }
    /// Merge another VizGroup into this one. The other group is drained and cannot be used again.
    pub fn merge(&mut self, other: &mut VizGroup) {
        assert_eq!(self.grid, other.grid);
        //  Drop all dead blocks before merging.
        other.live_blocks_weak.retain(|v| v.upgrade().is_some());
        //  Do the merge,
        self.live_blocks_weak.append(&mut other.live_blocks_weak);
        <Vec<RegionData> as AsMut<Vec<RegionData>>>::as_mut(&mut self.regions)
            .append(&mut other.regions);
        log::debug!(
            "Merged: {} live blocks weak, {} regions",
            self.live_blocks_weak.len(),
            self.regions.len()
        );
    }
}

/// Array of completed groups for one grid.
pub type CompletedGroups = Vec<Vec<RegionData>>;

/// Vizgroups - find all the visibility groups
pub struct VizGroups {
    /// The active column
    column: Vec<Rc<RefCell<LiveBlock>>>,
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
    pub fn new(detect_corners_touching: bool) -> Self {
        Self {
            column: Vec::new(),
            prev_region_data: None,
            completed_groups: Rc::new(RefCell::new(Vec::new())),
            live_blocks: LiveBlocks::new(),
            tolerance: if detect_corners_touching { 1 } else { 0 },
        }
    }
    
    /// Reset to ground state.
    /// Done after each grid.
    pub fn clear(&mut self) {
        self.column = Vec::new();
        self.prev_region_data = None;
        self.completed_groups = Rc::new(RefCell::new(Vec::new()));
        self.live_blocks = LiveBlocks::new();
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
                    if prev.1.borrow().xy_adjacent(curr, self.tolerance) {
                        prev.1.borrow_mut().blocks_touch(curr)
                    }

                    if curr.borrow().region_data.region_coords_y
                        < prev.1.borrow().region_data.region_coords_y
                    {
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
    fn end_column(&mut self) {
        //  If two live blocks in this list overlap, merge their viz groups.
        //  This is the check for overlap in Y.
        let mut prev_opt: Option<Rc<RefCell<LiveBlock>>> = None;
        for item in &mut self.column {
            if let Some(prev) = prev_opt {
                assert!(
                    prev.borrow().region_data.region_coords_y
                        <= item.borrow().region_data.region_coords_y,
                    "VizGroup data not sorted into increasing order in Y"
                );
                if prev.borrow().y_adjacent(item, self.tolerance) {
                    prev.borrow_mut().blocks_touch(item)
                }
            }
            prev_opt = Some(item.clone());
        }
        //  Next, need the check for overlap in X, between existing live blocks
        //  and new live blocks
        self.check_overlap_live_block_columns();

        //  Update the list of live blocks.
        //  Ones that ended at the column edge disappear.
        //  All new ones are added.
        log::debug!("End column. {} regions.", self.column.len());
        if !self.column.is_empty() {
            //  Purge now-dead live blocks. This will be all of them on SL, but wide regions on OS may not be ready to die yet.
            let x_limit = self.column[0].borrow().region_data.region_coords_x;
            self.live_blocks.purge_below_x_limit(x_limit);
            //  Add new live blocks.
            //  Put all the blocks in the column into the B-tree of live blocks.
            while let Some(b) = self.column.pop() {
                let y = b.borrow().region_data.region_coords_y;
                self.live_blocks.live_blocks.insert(y, b);
            }
            log::debug!("{} live blocks", self.live_blocks.live_blocks.len());
            assert!(self.column.is_empty());
        }
        self.column.clear();
    }

    /// End of input for one grid. Returns completed groups.
    pub fn end_grid(&mut self) -> CompletedGroups {
        //  Finish last column
        self.end_column();
        //  Flush all waiting live blocks.
        self.live_blocks.purge_below_x_limit(u32::MAX);
        log::info!("End grid.");
        let result = self.completed_groups.take();
        self.clear();
        result
    }

    /// Add one item of region data.
    /// Regions must be sorted by X, Y.
    /// It is not correct to have two overlapping regions, but we don't consider that fatal
    /// because sometimes the region database is temporarily inconsistent.
    pub fn add_region_data(&mut self, region_data: RegionData)  -> Option<CompletedGroups> {
        let mut result = None;
        if let Some(prev) = &self.prev_region_data {
            if region_data.grid != prev.grid {
                self.end_column();
                result = Some(self.end_grid());
            } else if region_data.region_coords_x != prev.region_coords_x {
                assert!(
                    region_data.region_coords_x >= prev.region_coords_x,
                    "VizGroup data not sorted into increasing order in X"
                );
                self.end_column();
            }
        };
        //  Add to column, or start new column.
        self.column.push(LiveBlock::new(
            &region_data,
            &Rc::<RefCell<Vec<Vec<RegionData>>>>::downgrade(&self.completed_groups),
        ));
        self.prev_region_data = Some(region_data);
        result
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
    const TEST_PATTERN: [(&str, u32, u32, u32, u32, &str); 24] = [
        ("Test", 0, 0, 100, 100, "Bottom left"),
        ("Test", 0, 100, 100, 100, "Left 100"),
        ("Test", 0, 200, 100, 100, "Left 200"),
        ("Test", 0, 300, 100, 100, "Left 300"),
        ("Test", 0, 400, 100, 100, "Left 400"),
        ("Test", 100, 0, 100, 100, "Bottom 100"),
        ("Test", 200, 0, 100, 100, "Bottom 200"),
        ("Test", 200, 300, 100, 100, "Tiny West"),
        ("Test", 300, 0, 100, 100, "Bottom 300"),
        ("Test", 300, 300, 100, 100, "Tiny East"),
        ("Test", 400, 0, 100, 100, "Bottom 400"),
        ("Test", 500, 0, 100, 100, "Bottom 500"),
        ("Test", 500, 100, 100, 100, "Column 5-1"),
        ("Test", 500, 200, 100, 100, "Column 5-2"),
        ("Test", 500, 300, 100, 100, "Column 5-3"),
        ("Test", 500, 400, 100, 100, "Column 5-4"),
        ("Test", 600, 400, 100, 100, "Top 600"),
        ("Test", 700, 100, 100, 200, "Tall skinny region"),
        ("Test", 700, 400, 100, 100, "Top 700"),
        ("Test", 800, 400, 100, 100, "Top 800"),
        ("Test", 900, 100, 100, 100, "Right 100"),
        ("Test", 900, 200, 100, 100, "Right 200"),
        ("Test", 900, 300, 100, 100, "Right 300"),
        ("Test", 900, 400, 100, 100, "Right 400"),
    ];

    let test_data: Vec<_> = TEST_PATTERN
        .iter()
        .map(
            |(grid, region_coords_x, region_coords_y, size_x, size_y, name)| RegionData {
                grid: grid.to_string(),
                region_coords_x: *region_coords_x,
                region_coords_y: *region_coords_y,
                size_x: *size_x,
                size_y: *size_y,
                name: name.to_string(),
            },
        )
        .collect();
    //  All errors to console
    simplelog::CombinedLogger::init(
        vec![
            simplelog::TermLogger::new(simplelog::LevelFilter::Trace, simplelog::Config::default(), 
            simplelog::TerminalMode::Mixed, simplelog::ColorChoice::Auto),
        ]
    ).unwrap();
    let mut viz_groups = VizGroups::new(false);
    for item in test_data {
        let grid_break = viz_groups.add_region_data(item);
        //  This example is all one grid, so there's no control break.
        assert_eq!(grid_break, None);
    }
    let results = viz_groups.end_grid();
    //  Display results
    log::info!(
        "Result: Viz groups: {}",
        results.len()
    );
    for viz_group in results.iter() {
        log::info!("Viz group: {:?}", viz_group);
    }
    assert_eq!(results.len(), 3); // 3 groups in this test case.
}
