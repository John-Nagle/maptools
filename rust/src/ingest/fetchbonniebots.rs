//! fetchbonniebots.rs -- obtain Second Life / Open Simulator terrain data from bonniebots.
//!
//! Part of the Animats impostor system.
//!
//! Fetches elevation data from BonnieBots.
//! 
//! Data is fetched from URLs of the form:
//!
//! https://www.bonniebots.com/static-api/terrain/1029-909.bin
//!
//! The file is a 256x226 array of floats, with no padding.
//!
//!     License: LGPL.
//!     Animats
//!     April, 2026.
//!
///
use anyhow::{ Error, anyhow };
/// Size of region elev data, SL only.
const TERRAIN_DATA_SIZE: usize = 256*256;
/// User agent for talking to asset server
const USER_AGENT: &str = "animats.info impostor asset system";
/// Fetch elevation data from Bonniebots. 256x256.
pub fn fetch_elevs(region_num_x: u32, region_num_y: u32) -> Result<Option<[f32;TERRAIN_DATA_SIZE]>, Error> {
    /// Build URL
    let url = format!("https://www.bonniebots.com/static-api/terrain/{}-{}.bin", region_num_x, region_num_y);
    todo!();
}
#[test]
fn test_fetchelevs() {
    //  All errors to console
    use common::test_logger;
    test_logger();
    let elevs = fetch_elevs(1000, 1000).expect("Fetch from BonnieBots failed.");
 }
