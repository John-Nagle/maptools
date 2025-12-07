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
use std::io::{Write, Cursor};
use std::path::PathBuf;

use crate::vizgroup::{CompletedGroups, RegionData, VizGroups};
use image::{RgbImage, DynamicImage, ImageReader};


/// Advance across a LOD one column at a time.
pub struct ColumnCursor {
}

impl ColumnCursor {
    /// Advance column if possible
    pub fn advance(&mut self) {
        //  ***MORE***
        todo!();
    }
    
    /// True if advance is safe. That is, the previous LOD columns needed
    /// to build this column are already done.
    fn is_advance_safe(&self) -> bool {
        //  ***MORE***
        todo!();
    }
}
