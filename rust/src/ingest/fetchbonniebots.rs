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
use anyhow::{ Error, anyhow };
/// Size of region elev data, SL only.
const TERRAIN_DATA_SIZE: usize = 256*256;
/// User agent for talking to asset server
const USER_AGENT: &str = "animats.info impostor asset system";
/// Fetch elevation data from Bonniebots. 256x256.
pub fn fetch_elevs(agent: &mut Agent, region_num_x: u32, region_num_y: u32) -> Result<Option<[f32;TERRAIN_DATA_SIZE]>, Error> {
    /// Build URL
    let url = format!("https://www.bonniebots.com/static-api/terrain/{}-{}.bin", region_num_x, region_num_y);        
    match ureq::get(&url).call() {
        Ok(mut response) => {
            let content = response.body_mut().read_to_vec()?;
            log::debug!("Length of content: {}", content.len());
            Ok(None)    // ***TEMP***
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
#[test]
fn test_fetchelevs() {
    //  All errors to console
    use common::test_logger;
    test_logger();
    //  HTTP Agent
    let mut config = Agent::config_builder()       
        .build();
    let mut agent: Agent = config.into();
    let elevs = fetch_elevs(&mut agent, 1000, 1000).expect("Fetch from BonnieBots failed.");
 }
