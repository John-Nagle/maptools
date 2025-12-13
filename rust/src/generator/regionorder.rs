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
/// constructe higher LOD impostors.

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
        let cursors: Vec<_> = (0..MAX_LOD).map(|lod| ColumnCursor::new(bounds, lod)).collect();
        Self {
            bounds, 
            regions,
            cursors
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
        todo!();
    }
}

#[derive(Default, Clone, Copy, Debug)]
enum RecentRegionType {
    /// Not checked yet
    #[default] Unknown,
    /// Empty water
    Water,
    /// Land
    Land,
    /// Error - should not happen
    Error,
}

/// The last two colums.
/// This is how we decide which lower LODs get impostered,
/// and when the info for them is emitted.
#[derive(Debug)]
pub struct RecentColumnInfo {
    /// Impostor size. Multiple regions. Meters.
    size:  (u32, u32),
    /// Offset of first entry. Meters.
    start: (u32, u32),
    /// Region type info
    region_type_info: [Vec<RecentRegionType>;2],
}

impl RecentColumnInfo {
    /// New. Sizes the recent column info for one LOD and
    /// fills in the array with Unknown.
    pub fn new(
        bounds: ((u32, u32), (u32, u32)),
        region_size: (u32, u32),
        lod: u8,) -> Self {
        let (start, size) = get_group_scan_limits(bounds, region_size, lod);
        let (ll, ur) = bounds;
        let x_steps = (ur.0 - ll.0) / size.0 + 1;
        let region_type_info = [vec![RecentRegionType::Unknown; x_steps as usize], vec![RecentRegionType::Unknown; x_steps as usize]];
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
    
    //  ***MORE***    
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
            return RecentRegionType::Error
        };
        //  Return element.
        assert_eq!(y % self.size.1 ,0);
        row[(y / self.size.1) as usize]
    }
}

/// Advance across a LOD one column at a time.
pub struct ColumnCursor {}

impl ColumnCursor {
    /// Usual new
    pub fn new(bounds: ((u32, u32), (u32, u32)), lod: u8) -> ColumnCursor {
        todo!();
    }
    /// Advance column if possible
    pub fn advance(&mut self) {
        //  ***MORE***
        todo!();
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
