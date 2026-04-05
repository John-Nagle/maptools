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
use ureq::Agent;
use html_parser::{Dom};
use anyhow::{ Error, anyhow };
/// Size of region elev data, SL only.
const TERRAIN_DATA_SIZE: usize = 256*256;
/// User agent for talking to asset server
const USER_AGENT: &str = "animats.info impostor asset system";
/// Fetch elevation data from Bonniebots. 256x256.
pub fn fetch_elevs(agent: &mut Agent, region_num_x: u32, region_num_y: u32) -> Result<Option<Vec<f32>>, Error> {
    // Build URL
    let url = format!("https://www.bonniebots.com/static-api/terrain/{}-{}.bin", region_num_x, region_num_y);        
    match ureq::get(&url).call() {
        Ok(mut response) => {
            let content = response.body_mut().read_to_vec()?;
            log::debug!("Length of content: {}", content.len());
            const FLOAT_SIZE: usize = 4;    // this is a constant somewhere
            if content.len() != TERRAIN_DATA_SIZE*FLOAT_SIZE {
                Err(anyhow!("Elevation data has wrong size: {}. Expected {}. URL: {}", content.len(), TERRAIN_DATA_SIZE*FLOAT_SIZE, url))
            } else {
                let elevs: Vec<_> = content.chunks(FLOAT_SIZE).map(|c: &[u8]| f32::from_le_bytes(c.try_into().unwrap())).collect();
                log::debug!("Elevs: {:?}", &elevs[0..4]);  // ***TEMP***
                Ok(Some(elevs))    // ***TEMP***
            }
        }
        Err(ureq::Error::StatusCode(code)) => {
           // the server returned an unexpected status
            match code {
                404 => { Ok(None) }
                _ => { Err(anyhow!("HTTP error {} reading {}", code, url)) }
            }
        }
        Err(e) => Err(e.into())
    }
}

/// Info BonnieBots can provide for one region, from the region list.
pub struct BonnieBotsRegion {
}

/// Fetch region list from BonnieBots
pub fn fetch_region_list(agent: &mut Agent) -> Result<Vec<BonnieBotsRegion>, Error> {
    const BONNIEBOTSREGIONURL: &str = "https://www.bonniebots.com/region";
    let url = BONNIEBOTSREGIONURL;
    match ureq::get(url).call() {
        Ok(mut response) => {
            let content = response.body_mut().read_to_string()?;
            log::debug!("Length of content: {}", content.len());
            let dom = Dom::parse(&content)?;
            for child in dom.children {
                log::debug!("Child: {:?}", child);    // ***TEMP***
            }
            Ok(Vec::new())
        }
        Err(ureq::Error::StatusCode(code)) => {
            // the server returned an unexpected status
            Err(anyhow!("HTTP error {} reading {}", code, url))
        }
        Err(e) => Err(e.into())
    }
}

#[test]
/// Fetch all elevations for a single region.
fn test_fetchelevs() {
    //  All errors to console
    use common::test_logger;
    test_logger();
    //  HTTP Agent
    let config = Agent::config_builder()       
        .build();
    let mut agent: Agent = config.into();
    let elevs = fetch_elevs(&mut agent, 1000, 1000).expect("Fetch elevations from BonnieBots failed.");
 }
 
#[test]
/// Fetch all regions
fn test_fetchregions() {
    //  All errors to console
    use common::test_logger;
    test_logger();
    //  HTTP Agent
    let config = Agent::config_builder()       
        .build();
    let mut agent: Agent = config.into();
    let regions = fetch_region_list(&mut agent).expect("Fetch regions from BonnieBots failed.");
 }
