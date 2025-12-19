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

const MAX_LOD: u8 = 10;

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
}

impl ColumnCursors {
    /// The cursors for the regions.
    pub fn new(regions: Vec<RegionData>) -> Self {
        let bounds = get_group_bounds(&regions).expect("Invalid group bounds");
        assert!(!regions.is_empty()); // This is checked in get_group_bounds
        let base_region_size = (regions[0].size_x, regions[0].size_y);
        let cursors: Vec<_> = (0..MAX_LOD)
            .map(|lod| ColumnCursor::new(bounds, base_region_size, lod))
            .collect();
        Self {
            bounds,
            regions,
            cursors,
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
                match advance_status {
                    AdvanceStatus::None => continue,
                    AdvanceStatus::Data(region) => return Some(region),
                    AdvanceStatus::Retry => {
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
    /// Error - should not happen
    Error,
}

/// The last two columns.
/// This is how we decide which lower LODs get impostered,
/// and when the info for them is emitted.
#[derive(Debug)]
pub struct RecentColumnInfo {
    /// Impostor size. Multiple regions. Meters.
    size: (u32, u32),
    /// Offset of first entry. Meters.
    start: (u32, u32),
    /// Region type info
    region_type_info: [Vec<RecentRegionType>; 2],
}

impl RecentColumnInfo {
    /// New. Sizes the recent column info for one LOD and
    /// fills in the array with Unknown.
    pub fn new(bounds: ((u32, u32), (u32, u32)), region_size: (u32, u32), lod: u8) -> Self {
        let (start, size) = get_group_scan_limits(bounds, region_size, lod);
        let (ll, ur) = bounds;
        let x_steps = (ur.0 - ll.0) / size.0 + 1;
        let region_type_info = [
            vec![RecentRegionType::Unknown; x_steps as usize],
            vec![RecentRegionType::Unknown; x_steps as usize],
        ];
        log::debug!("LOD {}, {} steps", lod, x_steps);
        Self {
            start,
            size,
            region_type_info,
        }
    }

    /// Shift recent column info from current to previous column.
    /// Current column is 0, previous column is 1.
    fn shift(&mut self) {
        self.region_type_info[1] = self.region_type_info[0].clone();
        self.region_type_info[0] = vec![RecentRegionType::Unknown; self.region_type_info[0].len()];
        //  Advance position. Position is of the current column, not the previous one.
        self.start.0 += self.size.0;
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
            log::error!("Test cell for {:?} out of range in X for {:?}", loc, self);
            return RecentRegionType::Error;
        };
        //  Return element.
        assert_eq!(y % self.size.1, 0);
        row[(y / self.size.1) as usize]
    }

    /// Test a 4-cell quadrant for status.
    /// This is used by the next lowest LOD to decide what to do.
    fn test_four_cells(&self, loc: (u32, u32)) -> RecentRegionType {
        let (x, y) = loc;
        let s00 = self.test_cell((x, y));
        let s01 = self.test_cell((x, y + self.size.1));
        let s10 = self.test_cell((x + self.size.0, y));
        let s11 = self.test_cell((x + self.size.0, y + self.size.1));
        if (s00 == RecentRegionType::Error)
            || (s01 == RecentRegionType::Error)
            || (s10 == RecentRegionType::Error)
            || (s11 == RecentRegionType::Error)
        {
            return RecentRegionType::Error;
        }
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

enum AdvanceStatus {
    /// None - EOF
    None,
    /// Retry - go do this again
    Retry,
    /// Output - we have a result to return
    Data(RegionData),
}

/// Advance across a LOD one column at a time.
pub struct ColumnCursor {
    /// The last two columns.
    recent_column_info: RecentColumnInfo,
    /// Current location for this LOD. One rectangle
    /// past the last one filled in.
    next_loc: (u32, u32),
    /// Index into region data, for LOD 0 only
    region_data_index: usize,
}

impl ColumnCursor {
    /// Usual new
    pub fn new(
        bounds: ((u32, u32), (u32, u32)),
        base_region_size: (u32, u32),
        lod: u8,
    ) -> ColumnCursor {
        let size_mult = 2_u32.pow(lod as u32);
        let region_size = (
            base_region_size.0 * size_mult,
            base_region_size.1 * size_mult,
        );
        let recent_column_info = RecentColumnInfo::new(bounds, region_size, lod);
        let next_loc = recent_column_info.start;
        Self {
            recent_column_info,
            next_loc,
            region_data_index: 0,
        }
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
            "Mark {:?} as land. Size {:?}",
            loc,
            self.recent_column_info.size
        );
        assert!(self.recent_column_info.start.0 <= loc.0); // columns (X) must be in order
        while self.recent_column_info.start.0 < loc.0 {
            self.recent_column_info.shift();
            return false; // a shift occured, we will not do the insert
        }
        //  ***ADJUST COLUMN HERE***MORE***
        assert_eq!(self.recent_column_info.start.0, loc.0); // on correct column
        let yixloc = loc.1 / self.recent_column_info.size.1;
        assert_eq!(loc.1 % self.recent_column_info.size.1, 0);
        let yixstart = self.recent_column_info.start.1 / self.recent_column_info.size.1;
        assert!(yixloc >= yixstart);
        let yix = (yixloc - yixstart) as usize;
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
        self.recent_column_info.region_type_info[0][yix] = RecentRegionType::Land;
        true
        //////todo!();
    }

    /// Advance to next region, for LOD 0 only.
    pub fn advance_lod_0(&mut self, regions: &Vec<RegionData>) -> AdvanceStatus {
        let n = self.region_data_index;
        if n < regions.len() {
            let region = &regions[n];
            let loc = (region.region_coords_x, region.region_coords_y);
            if !self.mark_as_land(loc) {
                // We advanced a row, and must do this again.
                return AdvanceStatus::Retry;
            }
            self.region_data_index += 1;
            AdvanceStatus::Data(region.clone())
        } else {
            //  End of input
            AdvanceStatus::None
        }
    }

    /// Advance to next region, for LOD > 0.
    pub fn advance_lod_n(&mut self, recent_column_info: &RecentColumnInfo) -> AdvanceStatus {
        return AdvanceStatus::None; // ***TEMP TURNOFF*** just do LOD 0
    }

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

/// For a group with given bounds, find the starting point and increments which will step
/// a properly aligned rectangle for the given LOD over the bounds covering all rectangles within the bounds.
/// This is pure math.
pub fn get_group_scan_limits(
    bounds: ((u32, u32), (u32, u32)),
    region_size: (u32, u32),
    lod: u8,
) -> ((u32, u32), (u32, u32)) {
    //  Get lower left and upper right
    let (lower_left, _upper_right) = bounds;
    let lod_mult = 2_u32.pow(lod as u32);
    let step = (region_size.0 * lod_mult, region_size.1 * lod_mult);
    //  Now the tricky part. Round down the lower_left values to the next lower multiple of step.
    //  ***UNTESTED***
    let start = (
        (lower_left.0 / step.0) * step.0,
        (lower_left.1 / step.1) * step.1,
    );
    (start, step)
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
        let mut prev_loc_opt = None;
        for item in &group {
            let loc = (item.region_coords_x, item.region_coords_y);
            if let Some(prev_loc) = prev_loc_opt {
                check_loc_sequence(prev_loc, loc);
            }
            prev_loc_opt = Some(loc);
        }
        //  Do test for one group
        let mut column_cursors = ColumnCursors::new(group);
        for item in column_cursors {
            log::debug!("Item: {:?}", item);
        }
        // ***MORE***
    }
}
