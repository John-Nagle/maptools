//! regionorder.rs -- generate Second Life / Open Simulator terrain objects as files to be uploaded.
//! Part of the Animats impostor system
//!
//! When generating impostor objects, we have a memory problem. The largest
//! viz group in Second Life has about 28,000 regions. The data for each region
//! includes a height map and at least two images of around a quarter megabyte each.
//! Plus we generate the lower LOD images, only 1/4 as many, but still substantial.
//! This all adds up to about 20GB of RAM, and that's if we stick to 256x256 images.
//! So the brute force approach requires too much memory.
//!
//! The approach used is thus sequential processing. The region data is sorted by x,y
//! when it comes in from SQL. We work across the data by column. On each cycle, we
//! can do all the LOD 0 regions of the column. On every other cycle, the LOD 1 regions
//! that need two columns of the LOD 0 regions. On every fourth cycle, the LOD 2 regions
//! that need two columns of the LOD 1 regions. And so forth.
//!
//! For what little this does, it is unreasonably complicated.
//!
//!     License: LGPL.
//!     Animats
//!     December, 2025.
//
use anyhow::{anyhow, Error};
use std::collections::VecDeque;
use crate::vizgroup::{RegionData};

/// Maximum LOD. It never gets this big, because there would have to be a viz group 2^LOD across for that to happen.
const MAX_LOD: u8 = 16;

/// All the column cursors for all the LODs.
///
/// The goal here is to return all the regions that
/// need to be impostored in the order that will allow
/// the lower LOD impostors to be constructed from recently
/// constructes higher LOD impostors.

pub struct TileLods {
    /// Cursors for each LOD
    cursors: Vec<ColumnCursor>,
    /// The regions in
    regions: VecDeque<RegionData>,
    /// Available results
    regions_to_output: VecDeque<RegionData>,
}

impl TileLods {
    /// The cursors for the levels of detail of regions.
    pub fn new(mut regions: Vec<RegionData>) -> Self {
        let bounds = get_group_bounds(&regions).expect("Invalid group bounds");
        log::debug!("Group bounds: {:?}", bounds);
        assert!(!regions.is_empty()); // This is checked in get_group_bounds
        //  Sort by X, Y. The input is usually almost in order, but not quite.
        regions.sort_by_key(|v: &RegionData| (v.region_coords_x, v.region_coords_y));
        //  Immutable after this point
        let regions = regions;
        let base_region_size = (regions[0].size_x, regions[0].size_y);
        let (max_lod, ll,ur) = get_group_scan_bounds(bounds, base_region_size).expect("Group scan bounds calc failed");
        //  ***CHECK FOR AT LEAST 2X2***
        //  ***MUST HAVE AS MANY COLUMNS AS ROWS*** add columns if necessary
        let grid = &regions[0].grid;
        //  Generate LODs unti one LOD covers the entire bounds.
        let mut cursors = Vec::new();
        for lod in 0..(max_lod+1) {
            let new_cursor = ColumnCursor::new((ll, ur), base_region_size, lod, grid.clone());
            let done = new_cursor.recent_column_info.is_full_coverage();
            cursors.push(new_cursor);
            if done {
                break
            }
        }
        //  Must have at least 4 cells or lower LODs are impossible.
        assert!(cursors.len() > 1);
        assert!(cursors[0].recent_column_info.region_type_info[0].len() > 1);
        Self {
            regions: regions.into(),
            cursors,
            regions_to_output: VecDeque::new(),
        }
    }
    
    /// Scan for newly finished blocks. Then shift down by one column of LOD 0.
    fn scan_and_shift(&mut self) {
        //  Advance by exactly one column.
        //  Finish out current column.
        self.cursors[0].column_finished();
        //  Process lower LODs with current alignment.
        for lod in 1..self.cursors.len() {
            let (prev, curr) = self.cursors.split_at_mut(lod as usize);
            assert!(!prev.is_empty());
            let prev = &prev[prev.len() - 1];
            let curr: &mut ColumnCursor = &mut curr[0];
            log::debug!("Scan LOD {}. Prev: {:?}  Curr: {:?}", lod, prev.recent_column_info, curr.recent_column_info); // ***TEMP***
            if !curr.is_aligned(&prev.recent_column_info) { break };
            let mut new_tiles = curr.scan_lod_n(&prev.recent_column_info);
            //  A lower LOD region has been generated.
            self.regions_to_output.append(&mut new_tiles);
        }
        //  Done looking at lower LODs, now shift and align all LODs.
        //  LOD 0 always gets shifted at least once.
        self.cursors[0].shift();    // Shift LOD 0
        //  Now shift the lower LODs, if this will bring them into alignment.
        //  LOD 1 gets shifted one in two times.
        //  LOD 2 gets shifted one in four times, etc.
        for lod in 1..self.cursors.len() {         
            let can_shift = self.cursors[0].recent_column_info.start.0 == self.cursors[lod].recent_column_info.start.0 + self.cursors[lod].recent_column_info.size.0;
            log::debug!("Scan and shift, LOD {}, lod 0 x at {}, LOD {} x at {}, size {}, can shift: {}",
                lod, self.cursors[0].recent_column_info.start.0, lod, 
                self.cursors[lod].recent_column_info.start.0, self.cursors[lod].recent_column_info.size.0, can_shift);
            if can_shift {
                self.cursors[lod].shift();    // Shift LOD N
                //  Should now be aligned.
                assert_eq!(self.cursors[0].recent_column_info.start.0, self.cursors[lod].recent_column_info.start.0);
            } else {
                break;
            }
        }
    }
}

impl Iterator for TileLods {
    type Item = RegionData;
    /// Next, new version
    /// This is an iterator, which turns the loops inside out and means we have
    /// to maintain too much state.
    fn next(&mut self) -> Option<Self::Item> {
        //  First, return region queued to be returned from the iterator, if any.
        let region = self.regions_to_output.pop_front();
        if region.is_some() {
            return region
        }
        //  No region was queued to be returned.
        //  So get a new input region.
        if let Some(region) = self.regions.pop_front() {
            //  We have a new region to handle.
            //  Mark it in the current row.
            let loc = (region.region_coords_x, region.region_coords_y);
            if loc.0 == self.cursors[0].recent_column_info.start.0 {
                //  Column has not changed.
                self.cursors[0].mark_lod_0(loc);
                assert!(self.regions_to_output.is_empty());
                return Some(region);
            } else {
                //  Queue this region for output
                self.regions_to_output.push_back(region.clone());
                //  Column has changed.
                assert!(loc.0 > self.cursors[0].recent_column_info.start.0);
                while loc.0 > self.cursors[0].recent_column_info.start.0 {
                    self.scan_and_shift();
                }
                //  Aligned now, push new item.
                assert_eq!(loc.0, self.cursors[0].recent_column_info.start.0);
                self.cursors[0].mark_lod_0(loc);
            }
            //  There must always be a region queued at this point, because we pushed one at start.
            let opt_region = self.regions_to_output.pop_front();
            assert!(opt_region.is_some());
            opt_region
        } else {
            //  End of input.
            //  Lower LODs must be flushed.
            //  Mark entire column as water, then call scan and shift, until lowest LOD is completed.
            //  Done when the lowest LOD is completed.
            //  ***EOF TEST CAN RUN AWAY***
            let mut runaway: usize = 0; // ***TEMP***
            log::debug!("Runout start: lowest LOD is LOD {}", self.cursors.len()-1); 
            //  ***TERMINATION CONDITION MAY BE TOTALLY BOGUS TESTING AGAINST ROW 1***    
            self.scan_and_shift();      
            //  ***PROBABLY SHOULD BE STRICTLY LESS FOR LOOP TERMINATION TEST***
            while self.cursors[self.cursors.len()-1].recent_column_info.start.0 <= self.cursors[self.cursors.len()-1].recent_column_info.lod_bounds.1.0 {
                log::debug!("Runout at EOF: at {:?}", self.cursors[0].recent_column_info.start);
                log::debug!("Runout: next y index: {} for length {}", self.cursors[0].next_y_index, self.cursors[0].recent_column_info.region_type_info[0].len());
                log::debug!("Runout: Col finished LOD 0: {:?}", self.cursors[0].recent_column_info.region_type_info[0]);  // ***TEMP***
                //  This fills all with water.
                self.scan_and_shift();
                if runaway > 100 { panic!("EOF runaway"); } else { runaway += 1; } // ***TEMP***
            }
            log::debug!("Runout done"); 
            //  Return a region, or None if we're all done.
            self.regions_to_output. pop_front()
        }
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
enum RecentRegionType {
    /// Not checked yet
    #[default]
    Unknown,
    /// Empty water
    Water,
    /// Land
    Land,
}

/// The most recent two columns.
/// This is how we decide which lower LODs get impostered,
/// and when the info for them is emitted.
#[derive(Debug)]
pub struct RecentColumnInfo {
    /// Impostor tile size at this LOD. Meters.
    size: (u32, u32),
    /// Offset of first entry. Meters.
    start: (u32, u32),
    /// Bounds of the entire map at this LOD.
    lod_bounds: ((u32, u32), (u32, u32)),
    /// Region type info
    region_type_info: [Vec<RecentRegionType>; 2],
    /// True if this LOD needs only one tile to cover the entire area
    full_coverage: bool,
}

impl RecentColumnInfo {
    /// New. Sizes the recent column info for one LOD and
    /// fills in the array with Unknown.
    pub fn new(bounds: ((u32, u32), (u32, u32)), base_region_size: (u32, u32), lod: u8) -> Self {
        //  All columns have same bounds. Only the resolution differs.
        let ll = bounds.0;
        let ur = bounds.1;
        log::debug!("New recent column info, LOD{}: ur: {:?}, ll: {:?}, base_region_size: {:?}", lod, ur, ll, base_region_size);    // ***TEMP***
        let scale = 2_u32.pow(lod as u32);
        let tile_size = (
            base_region_size.0 * scale,
            base_region_size.1 * scale,
        );
        let y_steps = (ur.1 - ll.1) / tile_size.1;
        //  The off the edge row, row 1, starts as all water.
        let region_type_info = [
            vec![RecentRegionType::Unknown; y_steps as usize],
            vec![RecentRegionType::Water; y_steps as usize],
        ];        
        let lod_bounds = (ll, ur);
        let full_coverage = y_steps == 1;
        log::debug!("LOD {}, bounds {:?}, {} y_steps, full coverage: {}", lod, bounds, y_steps, full_coverage);

        Self {
            size: tile_size,	
            region_type_info,
            full_coverage,
            lod_bounds,
            start: lod_bounds.0,
        }
    }
    
    /// Calculate array index for a Y value.
    /// Non-fatal bounds check
    pub fn try_calc_y_index(&self, y: u32) -> Option<usize> {
        let ll_y = self.lod_bounds.0.1;
        if y >= ll_y {
            let yix = ((y - ll_y) / self.size.1) as usize;
            if yix < self.region_type_info[0].len() {
                Some(yix)
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// As above, but panic on bounds check
    pub fn calc_y_index(&self, y: u32) -> usize {
        if let Some(yix) = self.try_calc_y_index(y) {
            yix 
        } else {
            panic!("RegionOrder::RecentColumnInfo bounds check fail: y: {}, size: {:?}, start: {:?}, len: {}",
                y, self.size, self.lod_bounds, self.region_type_info[0].len());
        }
    }

    /// Shift recent column info from current to previous column.
    /// Current column is 0, previous column is 1.
    fn shift_inner(&mut self) {
        //  Columns must be totally filled in before a shift.
        log::trace!("Shift_inner: {:?}", self.region_type_info[0]); // ***TEMP***
        assert!(self.region_type_info[0].iter().find(|&&v| v == RecentRegionType::Unknown).is_none());
        self.region_type_info[1] = self.region_type_info[0].clone();
        self.region_type_info[0] = vec![RecentRegionType::Unknown; self.region_type_info[0].len()];
        //  Advance position. Position is of the current column, not the previous one.
        self.start.0 += self.size.0;
        log::debug!("Column shift. Next start: {:?}", self.start);
    }
    
    /// Does this tile cover the entire bounds of the viz group?
    fn is_full_coverage(&self) -> bool {
        self.full_coverage
    }   

    /// Test one cell for status
    fn test_cell(&self, loc: (u32, u32)) -> RecentRegionType {
        let (x, y) = loc;
        //  Check that X is within bounds.
        //  Low limit is previous column.
        //  High limit is current column.
        let column = if x == self.start.0 {
            &self.region_type_info[0]
        } else if x + self.size.0 == self.start.0 {
            &self.region_type_info[1]
        } else {
            log::trace!("Tested cell of invalid column: x: {}, column 0: {}, column 1: {}", x, self.start.0, self.start.0 as i32 - self.size.0 as i32);
            return RecentRegionType::Water;
        };
        //  Return element.
        assert_eq!(y % self.size.1, 0);
        //  If out of range, treat as water.
        let result = if let Some(v) = column.get(((y - self.start.1) / self.size.1) as usize) {
            *v
        } else {
            RecentRegionType::Water
        };
        log::trace!("Test cell: loc {:?}, yix: {}, result: {:?}", loc, y/self.size.1, result);
        result
    }

    /// Test a 4-cell quadrant for status.
    /// This is used by the next lowest LOD to decide what to do.
    fn test_four_cells(&self, loc: (u32, u32)) -> RecentRegionType {
        let (x, y) = loc;
        let s00 = self.test_cell((x, y));
        let s01 = self.test_cell((x, y + self.size.1));
        let s10 = self.test_cell((x + self.size.0, y));
        let s11 = self.test_cell((x + self.size.0, y + self.size.1));
        //  Unknown, can't process yet.
        if (s00 == RecentRegionType::Unknown)
            || (s01 == RecentRegionType::Unknown)
            || (s10 == RecentRegionType::Unknown)
            || (s11 == RecentRegionType::Unknown)
        {
            return RecentRegionType::Unknown;
        }
        //  All water, impostor as water.
        if (s00 == RecentRegionType::Water)
            && (s01 == RecentRegionType::Water)
            && (s10 == RecentRegionType::Water)
            && (s11 == RecentRegionType::Water)
        {
            return RecentRegionType::Water;
        }
        //  Not all water, but ready to process. Impostor as land.
        log::debug!("Found four cells with some land at {:?}", loc);
        RecentRegionType::Land
    }
}

/// Advance across a LOD one column at a time.
struct ColumnCursor {
    /// The last two columns.
    recent_column_info: RecentColumnInfo,
    /// Current location for this LOD. One rectangle
    /// past the last one filled in. LOD 0 only.
    next_y_index: usize,
    /// LOD
    lod: u8,
    /// Grid, for output
    grid: String,
}

impl ColumnCursor {
    /// Usual new
    fn new(
        bounds: ((u32, u32), (u32, u32)),
        base_region_size: (u32, u32),
        lod: u8,
        grid: String,
    ) -> ColumnCursor {
        //  Calculate tile size at this LOD.
        let recent_column_info = RecentColumnInfo::new(bounds, base_region_size, lod);
        Self {
            recent_column_info,
            next_y_index: 0,
            lod,
            grid,
        }
    }

    /// Mark individual region type
    fn mark_region_type(&mut self, yix: usize, recent_region_type: RecentRegionType) {
        log::trace!(
            "Mark LOD {} index {} as {:?}. Size {:?}",
            self.lod,
            yix,
            recent_region_type,
            self.recent_column_info.region_type_info[0].len()
        );
        assert_eq!(self.recent_column_info.region_type_info[0][yix], RecentRegionType::Unknown);
        self.recent_column_info.region_type_info[0][yix] = recent_region_type;
    }
    
    /// Shift recent column info from current to previous column.
    /// Current column is 0, previous column is 1.
    fn shift(&mut self) {
        self.column_finished();
        log::debug!("Shift LOD {} column finished: {:?}", self.lod, self.recent_column_info.region_type_info[0]); // ***TEMP***
        self.recent_column_info.shift_inner();
        self.next_y_index = 0;
    }
    
    /// Build a new tile for a LOD > 0.
    fn build_new_tile(&self, loc: (u32, u32), size: (u32, u32)) -> RegionData {
        //  Dummy name for higher LODs.
        let name = format!("LOD{} {:?}", self.lod, loc);
        //  Build a new tile.
        RegionData {
            grid: self.grid.clone(),
            region_coords_x: loc.0,
            region_coords_y: loc.1,
            size_x: size.0,
            size_y: size.1,
            name,
            lod: self.lod,
        }
    }
    
    /// Mark cell in use on LOD 0.
    fn mark_lod_0(&mut self, loc: (u32, u32)) {
        assert_eq!(self.lod, 0);    // LOD 0 only.
        assert_eq!(self.recent_column_info.start.0, loc.0); // on correct column
        let yix = self.recent_column_info.calc_y_index(loc.1);
        log::debug!("Marking cell {} of column {:?}",  yix, self.recent_column_info.region_type_info[0]);
        assert_eq!(loc.1 % self.recent_column_info.size.1, 0);
        //  Duplicates not allowed.
        assert_eq!(
            self.recent_column_info.region_type_info[0][yix],
            RecentRegionType::Unknown
        );
        //  Mark this as a land cell.
        //  Fill as water up to new land cell.
        log::debug!(
            "Mark {:?}, index {} as land. Size {:?}",
            loc,
            yix,
            self.recent_column_info.size
        );
        //  Fill as water up to, but not including, yix.
        for n in self.next_y_index .. yix {
            self.mark_region_type(n, RecentRegionType::Water);
        }
        self.mark_region_type(yix, RecentRegionType::Land);
        self.next_y_index = yix + 1;
    }
    
    /// Finished with this LOD 0 column. Fill out to end.
    fn column_finished(&mut self) {
        assert!(self.recent_column_info.region_type_info[0].len() > 0);
        let fill_last = self.recent_column_info.region_type_info[0].len() -1;
        log::debug!("Col finished LOD {} start, yix = {}: {:?}", self.lod, self.next_y_index, self.recent_column_info.region_type_info[0]);  // ***TEMP***
        if self.recent_column_info.region_type_info[0][fill_last] == RecentRegionType::Unknown {
            //  This column is not full yet, so we have to fill it out to the end.
            for n in self.next_y_index as usize .. fill_last + 1 {
                assert_eq!(self.recent_column_info.region_type_info[0][n], RecentRegionType::Unknown);
                self.recent_column_info.region_type_info[0][n] = RecentRegionType::Water;
            }
            //  At this point, all entries in the column should be known.
            log::debug!("Col finished LOD {} done, yix = {}: {:?}", self.lod, self.next_y_index, self.recent_column_info.region_type_info[0]);  // ***TEMP***
            
        }
        //  Column complete. All cells are land or water.
        assert!(self.recent_column_info.region_type_info[0].iter().find(|&&v| v == RecentRegionType::Unknown).is_none());
    }
    
    /// Scan all of a column of LOD n, returning any new tiles.
    fn scan_lod_n(&mut self, previous_lod_column_info: &RecentColumnInfo) -> VecDeque<RegionData> {
        assert!(self.is_aligned(previous_lod_column_info));
        let mut new_tiles = VecDeque::new();
        for n in 0..self.recent_column_info.region_type_info[0].len() {
            let loc = (self.recent_column_info.start.0, self.recent_column_info.start.1 + self.recent_column_info.size.1 * (n as u32));
            match previous_lod_column_info.test_four_cells(loc) {
                RecentRegionType::Unknown => {
                    //  The previous LOD should be completely filled in now.
                    panic!("Scan of LOD {}: found Unknown tile at index {} of {:?}", 
                        self.lod, n, previous_lod_column_info.region_type_info);
                }
                RecentRegionType::Land => {
                    //  Generate and return a land tile.
                    let new_tile = self.build_new_tile(loc, self.recent_column_info.size);
                    log::debug!("New tile: {:?}", new_tile);
                    self.mark_region_type(n, RecentRegionType::Land);
                    new_tiles.push_back(new_tile);
                }
                RecentRegionType::Water => {  
                    self.mark_region_type(n, RecentRegionType::Water);   
                }    
            }
        }
        new_tiles
    }
    
    /// Is this region aligned in column with the region above?
    /// If so, it is legitimate to update this LOD.
    
    fn is_aligned(&self, prev: &RecentColumnInfo) -> bool {
        //  Appropriate test is curr.start == prev.start - prev.size
        //  This is written as curr.start + prev.size == prev.start to avoid unsigned underflow.
        self.recent_column_info.start.0 + prev.size.0 == prev.start.0
    }
    
    /// Display region type info as string. Useful for debug.
    fn _to_string(&self) -> String {
        let mut s = String::new();
        for col in 0..1 {           
            for v in &self.recent_column_info.region_type_info[col] {
                s.push(match v {
                    RecentRegionType::Water => 'W',
                    RecentRegionType::Land => 'L',
                    RecentRegionType::Unknown => 'U'
                });      
            }
        }
        s.push('\n');
        s
    }
}

/// Is this group suitable for multiple-LOD processing?
/// ***NEED CHECK THAT GROUP IS AT LEAST 2x2***
pub fn homogeneous_group_size(group: &Vec<RegionData>) -> Option<(u32, u32)> {
    //  Return size of region if group is homogeneous. It always is in SL. For OS, we don't try to do multi-region impostors.
    if !group.is_empty() && group
        .iter()
        .find(|v| v.size_x != group[0].size_x || v.size_y != group[0].size_y)
        .is_none() {
            Some((group[0].size_x, group[0].size_y))
    } else {
        None
    }
}


/// Get dimensions of a group.
pub fn get_group_bounds(group: &Vec<RegionData>) -> Result<((u32, u32), (u32, u32)), Error> {
    //  Error if empty group.
    if group.is_empty() {
        return Err(anyhow!("Empty viz group"));
    }
    //  Error if group is not homogeneous. It always is in SL. For OS, we don't try to do multi-region impostors.
    if group
        .iter()
        .find(|v| v.size_x != group[0].size_x || v.size_y != group[0].size_y)
        .is_some()
    {
        return Err(anyhow!("Regions in a viz group are not all the same size"));
    }
    Ok((
        (
            group
                .iter()
                .fold(u32::MAX, |acc, v| acc.min(v.region_coords_x)),
            group
                .iter()
                .fold(u32::MAX, |acc, v| acc.min(v.region_coords_y)),
        ),
        (
            group
                .iter()
                .fold(u32::MIN, |acc, v| acc.max(v.region_coords_x + v.size_x)),
            group
                .iter()
                .fold(u32::MIN, |acc, v| acc.max(v.region_coords_y + v.size_y)),
        ),
    ))
}

/// Get the bounds of the area of interest.
/// This is expanded so that it's an aligned power of 2 square
/// in region indices, then scaled up by meters.
pub fn get_group_scan_bounds(
    bounds: ((u32, u32), (u32, u32)),
    base_region_size: (u32, u32),
) -> Result <(u8, (u32, u32), (u32, u32)), Error> {
    //  Get lower left and upper right.
    let (lower_left, upper_right) = bounds;
    //  Convert them to cell units.
    //  Lower left rounds down.
    let lower_left_ix = (
        (lower_left.0 / base_region_size.0),
        (lower_left.1 / base_region_size.1),
    );
    //  Upper right rounds up.
    let upper_right_ix = (
        ((upper_right.0 + base_region_size.0 - 1) / base_region_size.0),
        ((upper_right.1 + base_region_size.1 - 1) / base_region_size.1),
    );
    let (lod, ll_ix, ur_ix) = get_enclosing_square((lower_left_ix, upper_right_ix))?;
    //  Convert back to meters.
    let new_ll = (
        ll_ix.0 * base_region_size.0,
        ll_ix.1 * base_region_size.1,
    );
    let new_ur = (
        ur_ix.0 * base_region_size.0,
        ur_ix.1 * base_region_size.1,
    );
    //  We don't compute step here because it's computed for each LOD.
    Ok((lod, new_ll, new_ur))  
}

/// Compute the power of two square of cells which encloses the area of interest.
/// The final output is the lowest LOD cell.
/// This works in units of cells, not meters.
pub fn get_enclosing_square(
    bounds_ix: ((u32, u32), (u32, u32))) -> Result<(u8, (u32, u32), (u32, u32)), Error> {     
    let (lower_left_ix, upper_right_ix) = bounds_ix; 
    //  Try increasing LODs until we get one that works.
    for lod in 1..MAX_LOD {
        //  Size of square
        let square_size = 2_u32.pow(lod.into());
        //  First find the lower limits.
        let new_ll_ix = (
            (lower_left_ix.0 / square_size) * square_size,
            (lower_left_ix.1 / square_size) * square_size,
        );
        //  Next, trial upper limits
        let new_ur_ix = (
            new_ll_ix.0 + square_size,
            new_ll_ix.1 + square_size,
        );
        //  Check if encloses
        if new_ur_ix.0 < upper_right_ix.0 || new_ur_ix.1 < upper_right_ix.1 {
            continue
        }
        //  We have a winner. Check it.
        log::debug!("Enclosing square: LOD {}, square size {}.  {:?}, {:?}", lod, square_size, new_ll_ix, new_ur_ix);
        //  Sanity checks on the geometry.
        assert!(new_ll_ix.0 <= lower_left_ix.0);
        assert!(new_ll_ix.1 <= lower_left_ix.1);
        assert!(new_ur_ix.0 >= upper_right_ix.0);
        assert!(new_ur_ix.1 >= upper_right_ix.1);
        assert_eq!(new_ur_ix.0 - new_ll_ix.0, square_size);
        assert_eq!(new_ur_ix.1 - new_ll_ix.1, square_size);
        return Ok((lod, new_ll_ix, new_ur_ix))
    }
    return Err(anyhow!("Can't enclose the bounds {:?} with an alighed square of {}", bounds_ix, MAX_LOD))
}


//  Unit test
#[test]
/// Test region order
fn test_region_order() {
    //  Set up logging
    use common::test_logger;
    // Check loc order. Panic if error.
    // This module assumes everything is in strictly increasing sequence. So we check.
    fn check_loc_sequence(a: (u32, u32), b: (u32, u32)) {
        if a.0 > b.0 || (a.0 == b.0 && a.1 >= b.1) {
            panic!("Locations out of sequence: a {:?} >= b {:?}", a, b);
        }
    }

    test_logger();
    //  Build test data
    use super::vizgroup::{VizGroups, vizgroup_test_patterns};
    let test_data = vizgroup_test_patterns()[1].clone();
    let mut viz_groups = VizGroups::new(false);
    for item in test_data {
        let grid_break = viz_groups.add_region_data(item);
        //  This example is all one grid, so there's no control break.
        assert_eq!(grid_break, None);
    }
    let results = viz_groups.end_grid();
    //  Validate data is in increasing order.
    for group in results {
        log::debug!("Next group, {} items", group.len());
        let mut prev_loc_opt = None;
        for item in &group {
            let loc = (item.region_coords_x, item.region_coords_y);
            if let Some(prev_loc) = prev_loc_opt {
                check_loc_sequence(prev_loc, loc);
            }
            prev_loc_opt = Some(loc);
        }
        //  Do test for one group
        let tile_lods = TileLods::new(group);
        log::debug!("Generating lower LODs");
        for item in tile_lods {
            log::debug!(" Output item: {:?}", item);
        }
        // ***MORE***
    }
}
