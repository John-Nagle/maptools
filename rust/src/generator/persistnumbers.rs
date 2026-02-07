//! persistnumbers.rs -- generate Second Life / Open Simulator terrain objects as files to be uploaded.
//!
//! Part of the Animats impostor system
//!
//! This is an optimization to avoid re-uploading assets unnecessarily.
//!
//! Each time generateterrain is run, we get a new set of vizgroup numbers.
//! What's in which vizgroup is close to what was there before, but changes
//! to the regions may cause vizgroup numbers and contents to change.
//! So the goal here is to use the newly generated vizgroup numbers, but
//! when possible point them to existing tile assets.
//!
//! So we give each tile_asset an original_viz_group numbe when it is generated.
//! The tile asset name, which contains the viz_group number, always matches that.
//! On each run of generateterrain, we generate all new viz_group numbers.
//! (Viz_group numbers are ordered by viz_group member count, so they don't change much.)z_
//!
//! When new viz_group numbers are assigned to old assets, ...??? ***MORE***
//!
//!     License: LGPL.
//!     Animats
//!     February, 2025.
//
use anyhow::{anyhow, Error};
use std::collections::VecDeque;
use crate::vizgroup::{RegionData};
