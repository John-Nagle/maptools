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
use common::Credentials;
use common::{HeightField, UploadedRegionInfo};

use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::path::PathBuf;

use crate::vizgroup::{CompletedGroups, RegionData, VizGroups};
use image::{DynamicImage, ImageReader, RgbImage};

/// Maximum LOD. It never gets this big, because there would have to be a viz group 2^LOD across for that to happen.
const MAX_LOD: u8 = 16;

/// Simple version, without optimization.
/// Just iterates over &Vec<RegionData>.
/// No LODs other than 0.
pub struct SimpleColumnCursors {
    /// The regions
    regions: Vec<RegionData>,
    /// The cursor
    cursor: usize,
}

impl SimpleColumnCursors {
    /// The regions
    pub fn new(regions: Vec<RegionData>) -> Self {
        Self { regions, cursor: 0 }
    }
}

impl Iterator for SimpleColumnCursors {
    type Item = RegionData;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor < self.regions.len() {
            let i = self.cursor;
            self.cursor += 1;
            Some(self.regions[i].clone())
        } else {
            None
        }
    }
}

/// All the column cursors for all the LODs.
///
/// The goal here is to return all the regions that
/// need to be impostored in the order that will allow
/// the lower LOD impostors to be constructed from recently
/// constructes higher LOD impostors.

pub struct ColumnCursors {
    /// Bounds of the entire region data
    bounds: ((u32, u32), (u32, u32)),
    /// Cursors for each LOD
    cursors: Vec<ColumnCursor>,
    /// The regions
    regions: Vec<RegionData>,
    /// Iteration state, LOD we are working on
    working_lod: u8,
    /// Was anything marked?
    progress_made: bool,
}

impl ColumnCursors {
    /// The cursors for the regions.
    pub fn new(regions: Vec<RegionData>) -> Self {
        let bounds = get_group_bounds(&regions).expect("Invalid group bounds");
        log::debug!("Group bounds: {:?}", bounds);
        assert!(!regions.is_empty()); // This is checked in get_group_bounds
        let base_region_size = (regions[0].size_x, regions[0].size_y);
        let grid = &regions[0].grid;
        //  Generate LODs unti one LOD covers the entire bounds.
        let mut cursors = Vec::new();
        for lod in 0..MAX_LOD {
            let new_cursor = ColumnCursor::new(bounds, base_region_size, lod, grid.clone());
            let done = new_cursor.recent_column_info.is_full_coverage();
            cursors.push(new_cursor);
            if done {
                break
            }
        }
        Self {
            bounds,
            regions,
            cursors,
            working_lod: 0,
            progress_made: false,
        }
    }
}

impl Iterator for ColumnCursors {
    type Item = RegionData;

    /// The iterator for ColumnCursors.
    /// This returns the next RegionData for which an impostor is to be generated.
    /// A RegionData for LOD > 0 may not be returned until all four regions for the
    /// next higher LOD have been returned, or are known to be empty water regions.
    /// The RegionData for a LOD > 0 should be returned as soon as all the needed
    /// regions to build that group of four have been built.
    /// This is to avoid the need to keep huge numbers of region images in memory
    /// at one time.
    fn next(&mut self) -> Option<Self::Item> {
/*
        //  Look for the lowest LOD for which we can return an item.
        //  ***NEEDS MORE EXPLAINATION***
        'outer: loop {
            let mut need_retry = false;
            for lod in (0..self.cursors.len()).rev() {
                let advance_status = if lod == 0 {
                    self.cursors[0].advance_lod_0(&self.regions)
                } else {
                    //  We need to mutably access two elements of the same array.
                    let (prev, curr) = self.cursors.split_at_mut(lod);
                    assert!(!prev.is_empty());
                    let prev = &prev[prev.len() - 1];
                    let curr: &mut ColumnCursor = &mut curr[0];
                    curr.advance_lod_n(&prev.recent_column_info)
                };
                //  If we have a winner, return it.
                log::debug!("LOD {}, advance {:?}", lod, advance_status);
                match advance_status {
                    AdvanceStatus::None => continue,
                    AdvanceStatus::Data(region) => return Some(region),
                    AdvanceStatus::Progress => {
                        //  Retry at outer loop level
                        need_retry = true;
                        continue;  
                    } // Need to go around again. AT WHAT LODs?***
                }
            }
            if !need_retry {
                break 'outer;
            }
        }
        //  Can't advance on any LOD. Done.
        None
*/
        //  The main loop to find the next item to return.
        // - Hold working LOD on Data(item) and Progress (i.e. water). Set progress_made.
        // - Advance working_lod if None and progress_made.
        // - Reset working_lod to 0 if None and !progress_made.
        // - If hit lowest LOD (largest number) and progress_made not set, done.
        loop {
            //  EOF test
            if self.working_lod as usize >= self.cursors.len() {
                if self.progress_made {
                    self.progress_made = false;
                    self.working_lod = 0;
                    continue;
                }
                break None;
            }
            let advance_status = if self.working_lod == 0 {
                self.cursors[0].advance_lod_0(&self.regions)
            } else {
                //  We need to mutably access two elements of the same array.
                let (prev, curr) = self.cursors.split_at_mut(self.working_lod as usize);
                assert!(!prev.is_empty());
                let prev = &prev[prev.len() - 1];
                let curr: &mut ColumnCursor = &mut curr[0];
                curr.advance_lod_n(&prev.recent_column_info)
            };
            //  If we have a winner, return it.
            log::debug!("LOD {}, advance {:?}", self.working_lod, advance_status);
            match advance_status {
                AdvanceStatus::None => {
                    self.working_lod += 1;
                    continue;
                }
                AdvanceStatus::Data(region) => {
                    self.progress_made = true;
                    break Some(region);
                }
                AdvanceStatus::Progress => {
                    self.progress_made = true;
                    continue;
                }
            }
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
        let (ll, ur, size) = get_group_scan_limits(bounds, base_region_size, lod);
        log::debug!("New recent column info, LOD{}: ur: {:?}, ll: {:?}, base_region_size: {:?}", lod, ur, ll, base_region_size);    // ***TEMP***
        let scale = 2_u32.pow(lod as u32);
        let tile_size = (
            base_region_size.0 * scale,
            base_region_size.1 * scale,
        );
        let x_steps = (ur.0 - ll.0) / tile_size.0;
        let y_steps = (ur.1 - ll.1) / tile_size.1;
        let region_type_info = [
            vec![RecentRegionType::Unknown; x_steps as usize],
            vec![RecentRegionType::Unknown; x_steps as usize],
        ];        
        let lod_bounds = (ll, ur);
        let full_coverage = x_steps == 1 && y_steps == 1;
        log::debug!("LOD {}, bounds {:?}, {} x_steps, {} y_steps, full coverage: {}", lod, bounds, x_steps, y_steps, full_coverage);

        Self {
            size,	
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
    fn shift(&mut self) {
        //  Columns must be totally filled in before a shift.
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
        //  Low limit is previous row.
        //  High limit is current row.
        let row = if x == self.start.0 {
            &self.region_type_info[0]
        } else if x + self.size.0 == self.start.0 {
            &self.region_type_info[1]
        } else {
            return RecentRegionType::Water;
        };
        //  Return element.
        assert_eq!(y % self.size.1, 0);
        //  If out of range, treat as water.
        if let Some(v) = row.get((y / self.size.1) as usize) {
            *v
        } else {
            RecentRegionType::Water
        }
        //////row[(y / self.size.1) as usize]
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
        RecentRegionType::Land
    }
}

#[derive(Debug)]
enum AdvanceStatus {
    /// None -- did not mark anything
    None,
    /// Made some progress -- Marked something, go do this again
    Progress,
    /// Output -- we have a result to return
    Data(RegionData),
}

/// Advance across a LOD one column at a time.
pub struct ColumnCursor {
    /// The last two columns.
    recent_column_info: RecentColumnInfo,
    /// Current location for this LOD. One rectangle
    /// past the last one filled in.
    next_y_index: usize,
    /// Index into region data, for LOD 0 only
    region_data_index: usize,
    /// LOD
    lod: u8,
    /// Grid, for output
    grid: String,
}

impl ColumnCursor {
    /// Usual new
    pub fn new(
        bounds: ((u32, u32), (u32, u32)),
        base_region_size: (u32, u32),
        lod: u8,
        grid: String,
    ) -> ColumnCursor {
        //  Calculate tile size at this LOD.
        let size_mult = 2_u32.pow(lod as u32);
        let recent_column_info = RecentColumnInfo::new(bounds, base_region_size, lod);
        let next_loc = recent_column_info.start;
        Self {
            recent_column_info,
            next_y_index: 0,
            region_data_index: 0,
            lod,
            grid,
        }
    }
    /// Mark individual region type
    pub fn mark_region_type(&mut self, yix: usize, recent_region_type: RecentRegionType) {
        log::debug!(
            "Try to mark LOD {} index {} as {:?}. Size {:?}",
            self.lod,
            yix,
            recent_region_type,
            self.recent_column_info.region_type_info[0].len()
        );
        assert_eq!(self.recent_column_info.region_type_info[0][yix], RecentRegionType::Unknown);
        self.recent_column_info.region_type_info[0][yix] = recent_region_type;
    }
    
    /// Mark region as land.
    /// Not just the current cell, but the ones leading up to it.
    /// Previous untouched cells are marked as Water.
    /// This relies in input being processed in x,y order.
    /// If this returns false, the markng was not done and we have to retry.
    /// This occurs when a column is complete.
    pub fn mark_as_land(&mut self, loc: (u32, u32)) -> bool {
        //  The update must be applied to row 0 of recent column info.
        //  If the location does not match, the recent column info must
        //  be adjusted.
        log::debug!(
            "Try to mark {:?} as land. Size {:?}",
            loc,
            self.recent_column_info.size
        );
        assert!(self.recent_column_info.start.0 <= loc.0); // columns (X) must be in order
        if self.recent_column_info.start.0 < loc.0 {
            log::debug!("Column break.");
            //  Column break. First, is this column full yet?
            assert!(self.recent_column_info.region_type_info[0].len() > 0);
            let fill_last = self.recent_column_info.region_type_info[0].len() -1;
            if self.recent_column_info.region_type_info[0][fill_last] == RecentRegionType::Unknown {
                //  This column is not full yet, so we have to fill it out to the end.
                let fill_start = (loc.1 - self.recent_column_info.start.1) / self.recent_column_info.size.1 + 1;
                log::debug!("Fill start: {}, fill last: {}", fill_start, fill_last);
                for n in self.next_y_index as usize .. fill_last + 1 {
                    assert_eq!(self.recent_column_info.region_type_info[0][n], RecentRegionType::Unknown);
                    self.recent_column_info.region_type_info[0][n] = RecentRegionType::Water;
                }
                //  At this point, all entries in the column should be known.
                log::debug!("Col: {:?}", self.recent_column_info.region_type_info[0]);  // ***TEMP***
                assert!(self.recent_column_info.region_type_info[0].iter().find(|&&v| v == RecentRegionType::Unknown).is_none());
                //  We won't do the insert; have to go around again and let the lower LODs have a chance.
                self.next_y_index = fill_last;  // which is off the end by 1.
                return false                    
            } else {
                //  This column is already full, and other LODs have been processed, so we can shift columns.
                self.next_y_index = 0;
                self.recent_column_info.shift();
                log::debug!("Shifted columns. Col: {:?}", self.recent_column_info.region_type_info[1]);
            }
        }      

        assert_eq!(self.recent_column_info.start.0, loc.0); // on correct column
        let yix = self.recent_column_info.calc_y_index(loc.1);
        assert_eq!(loc.1 % self.recent_column_info.size.1, 0);
        //  Duplicates not allowed.
        assert_eq!(
            self.recent_column_info.region_type_info[0][yix],
            RecentRegionType::Unknown
        );
        //  Mark this as a land cell.
        //  ***SHOULD WE FILL IN CELLS SKIPPED AS WATER CELLS?*** ***YES*** fill up to one being set here.
        //  ***HOW DO WE FILL OUT END OF LINE?***
        //  ***- We know about end of line only when X advances.
        //  ***- Now we need to fill out the line, and let lower LODs run before doing the shift.
        //  ***- Design problem. Need a 2-step process***
        //  ***- Need to separate y-changed from mark as land.
        //  ***- When Y changes for LOD 0, need to shift, then go around the LODs again, then mark as land.
        //  ***  When Y changes for LOD > 0, ??? How does that work?
        //  ***  If row advance peeks ahead for advance_lod_0, is that good enough?
        //  ***  - Not sure.
        //  Fill as water up to new land cell.
        log::debug!(
            "Mark {:?}, index {} as land. Size {:?}",
            loc,
            yix,
            self.recent_column_info.size
        );
        //  Fill as water up to, but not including, yix.
        log::debug!("Filling as water from {} to {} exclusive.", self.next_y_index, yix);
        for n in self.next_y_index .. yix {
            self.mark_region_type(n, RecentRegionType::Water);
            //////assert_eq!(self.recent_column_info.region_type_info[0][n], RecentRegionType::Unknown);
            //////self.recent_column_info.region_type_info[0][n] = RecentRegionType::Water;
        }
        self.mark_region_type(yix, RecentRegionType::Land);
        //////assert_eq!(self.recent_column_info.region_type_info[0][yix], RecentRegionType::Unknown);
        //////self.recent_column_info.region_type_info[0][yix] = RecentRegionType::Land;
        self.next_y_index = yix + 1;
        true
    }

    /// Advance to next region, for LOD 0 only.
    pub fn advance_lod_0(&mut self, regions: &Vec<RegionData>) -> AdvanceStatus {
        let n = self.region_data_index;
        if n < regions.len() {
            let region = &regions[n];
            let loc = (region.region_coords_x, region.region_coords_y);
            if !self.mark_as_land(loc) {
                // We advanced a row, and must do this again.
                return AdvanceStatus::Progress;
            }
            self.region_data_index += 1;
            AdvanceStatus::Data(region.clone())
        } else {
            //  End of input
            AdvanceStatus::None
        }
    }
    
    /// Build a new tile for a LOD > 0.
    fn build_new_tile(&self, loc: (u32, u32), size: (u32, u32)) -> RegionData {
        RegionData {
            grid: self.grid.clone(),
            region_coords_x: loc.0,
            region_coords_y: loc.1,
            size_x: size.0,
            size_y: size.1,
            name: "???".to_string(),    // ***TEMP***
            lod: self.lod,
        }
    }

    /// Advance to next region, for LOD > 0.
    /// This constructs LOD N entries based on LOD N-1.
    pub fn advance_lod_n(&mut self, previous_lod_column_info: &RecentColumnInfo) -> AdvanceStatus {
        log::debug!("Advance LOD {}, next y {}, col {:?}", self.lod, self.next_y_index, self.recent_column_info.region_type_info[0]);  // ***TEMP***
        //  Check for out of columns. This is the EOF test.
        if self.recent_column_info.start.0 >= self.recent_column_info.lod_bounds.1.0 { 
            log::debug!("LOD EOF 2 test passed: {} vs {}", self.recent_column_info.start.0, self.recent_column_info.lod_bounds.1.0);
            return AdvanceStatus::None
        }
        //  Test for done in Y axis.
        if self.next_y_index >= self.recent_column_info.region_type_info[0].len() {
            self.recent_column_info.shift();
            self.next_y_index = 0;
            //  Test for done in X axis
            if self.recent_column_info.start.0 >= self.recent_column_info.lod_bounds.1.0 { 
                log::debug!("LOD EOF 1 test passed: {} vs {}", self.recent_column_info.start.0, self.recent_column_info.lod_bounds.1.0);
                return AdvanceStatus::None
            }
        }
      
        //  Test next cell along Y axis.
        let loc = (self.recent_column_info.start.0, self.recent_column_info.start.1 + self.recent_column_info.size.1 * (self.next_y_index as u32));
        match previous_lod_column_info.test_cell(loc) {
            RecentRegionType::Unknown => {
                //  Not ready to do this yet.
                //  Try above LODs, then try again
                AdvanceStatus::None
            }
            RecentRegionType::Land => {
                self.recent_column_info.region_type_info[0][self.next_y_index] = RecentRegionType::Land;
                //  Generate and return a land tile.
                let new_tile = self.build_new_tile(loc, self.recent_column_info.size);
                log::debug!("New tile: {:?}", new_tile);
                self.next_y_index += 1;
                AdvanceStatus::Data(new_tile)
            }
            RecentRegionType::Water => {            
                //  Mark as a water tile to be skipped.
                self.recent_column_info.region_type_info[0][self.next_y_index] = RecentRegionType::Water;
                self.next_y_index += 1;
                AdvanceStatus::Progress
            }
        }
    }
/*
    /// True if advance is safe. That is, the previous LOD columns needed
    /// to build this column are already done.
    //  ***NEED A DATA STRUCTURE FOR EACH LOD THAT TELLS US WHETHER
    //  ***EACH CELL IS ALREADY DONE, KNOWN EMPTY WATER, or NOT DONE YET***
    //  ***SO TWO BITS PER CELL.***
    //  ***MAP BELONGS TO NEXT HIGHER LOD
    //  ***SET WHEN NEXT RETURNS A VALUE***
    //  ***NEED TO KEEP ONLY THE LAST TWO COLUMNS?
    //  *** Probably. Just keep an array of two columns x column length in region units with bounds from bounds calc.
    //  *** When column advances, shift array.
    //  ***  Offset issue is complicated but bounds calc already does most of the work.
    //  ***ADVANCE - Check the four indicated entries.
    //  *** All done - generate this item.
    //  *** All empty water - set this item as empty water, skip returning item.
    //  *** All done or empty water - generate this item.
    //  *** All not done yet - return done for now.
    fn is_advance_safe(&self) -> bool {
        //  ***MORE***
        todo!();
    }
*/
}

/// Get dimensions of a group.
pub fn get_group_bounds(group: &Vec<RegionData>) -> Result<((u32, u32), (u32, u32)), Error> {
    //  Error if empty group.
    //  ***BEGIN TEMP***
    for v in group {
        log::debug!(" Region loc: ({}, {}), size: ({}, {})", v.region_coords_x, v.region_coords_y, v.size_x, v.size_y);
    }
    //  ***END TEMP***
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

/// For a group with given bounds, find the starting point and increments which will step
/// a properly aligned rectangle for the given LOD over the bounds covering all rectangles within the bounds.
/// This is pure math.
pub fn get_group_scan_limits(
    bounds: ((u32, u32), (u32, u32)),
    region_size: (u32, u32),
    lod: u8,
) -> ((u32, u32), (u32, u32), (u32, u32)) {
    //  Get lower left and upper right
    let (lower_left, upper_right) = bounds;
    let lod_mult = 2_u32.pow(lod as u32);
    let step = (region_size.0 * lod_mult, region_size.1 * lod_mult);
    //  Now the tricky part. Round down the lower_left values to the next lower multiple of step.
    let new_ll = (
        (lower_left.0 / step.0) * step.0,
        (lower_left.1 / step.1) * step.1,
    );
    let new_ur = (
        ((upper_right.0 + step.0) / step.0) * step.0,
        ((upper_right.1 + step.1) / step.1) * step.1,
    );
    (new_ll, new_ur, step)
}

/// Check loc order. Panic if error.
/// This module assumes everything is in strictly increasing sequence. So we check.
pub fn check_loc_sequence(a: (u32, u32), b: (u32, u32)) {
    if a.0 > b.0 || (a.0 == b.0 && a.1 >= b.1) {
        panic!("Locations out of sequence: a {:?} >= b {:?}", a, b);
    }
}

//  Unit test
#[test]
/// Test region order
fn test_region_order() {
    //  Set up logging
    use common::test_logger;
    test_logger();
    //  Build test data
    use super::vizgroup::vizgroup_test_patterns;
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
        let column_cursors = ColumnCursors::new(group);
        log::debug!("Generating lower LODs");
        for item in column_cursors {
            log::debug!(" Output item: {:?}", item);
        }
        // ***MORE***
    }
}
