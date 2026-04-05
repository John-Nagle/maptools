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
use html_parser::{Dom, Node};
use anyhow::{ Error, anyhow };
use std::io::Read;

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

/// Find element by ID. First find only.
pub fn find_element_by_id<'a>(nodes: &'a Vec<Node>, id_key: &str) -> Option<&'a Node> {
    //  One would think this boilerplate would be part of html_parser.
    //  But it's easier to write this than bring in some huge crate that does too much.
    for node in nodes {
        match node {
            Node::Element(e) => {
                if let Some(id) = &e.id {
                    if id == id_key {
                        //  Find
                        return Some(node)
                    }
                }
                //  Recurse
                if let Some(found) = find_element_by_id(&e.children, id_key) {
                    return Some(found)
                }
            }
            Node::Text(_s) => {}
            Node::Comment(_c) => {}
        } 
    }
    //  No find
    None
}

/// Info BonnieBots can provide for one region, from the region list.
#[derive(Debug, Clone)]
pub struct BonnieBotsRegion {
    /// Name of region
    region_name: String,
    /// Region location number X (regions, not meters)
    region_x: u32,
    /// Y
    region_y: u32,
}

impl BonnieBotsRegion {
    //  Usual new
    pub fn new(region_name: &json::JsonValue, region_x: &json::JsonValue, region_y: &json::JsonValue) -> Option<Self> {
        Some(Self {
            region_name: region_name.as_str()?.to_string(),
            region_x: region_x.as_u32()?,
            region_y: region_y.as_u32()?,
        })
    }

    //   Build from BonnieBots JSON data
    pub fn from_json(regions: &json::JsonValue) -> Result<Vec<BonnieBotsRegion>, Error> {
        let mut region_records = Vec::new();
        if let json::JsonValue::Array(regions_array) = regions {
            for region in regions_array {
                //  Region data is an array
                let region_name = &region[0];
                let region_x = &region[6];
                let region_y = &region[7];
                region_records.push(Self::new(region_name, region_x, region_y).ok_or_else(|| anyhow!("Bad JSON region value: {:?}", region))?);                
            };
        } else {
            return Err(anyhow!("Did not find array in regions JSON from Bonniebots."));
        }
        Ok(region_records)
    }

}

/// Fetch region list from BonnieBots.
/// This assumes a very specific page layout at BonnieBots.
/// An API would be better.
pub fn fetch_region_list_json(agent: &mut Agent) -> Result<json::JsonValue, Error> {
    const BONNIEBOTSREGIONURL: &str = "https://www.bonniebots.com/regions";
    let url = BONNIEBOTSREGIONURL;
    const NEXT_DATA: &str = "__NEXT_DATA__";
    match agent.get(url)
        //////.header("user-agent", "curl/7.81.0")
        .call() {
        Ok(mut response) => {
            //////let content = response.body_mut().read_to_string()?;
            let mut reader = response.body_mut().as_reader();
            let mut body = Vec::new();
            reader.read_to_end(&mut body)?; // Ensures all data is read
            let content = String::from_utf8(body)?;            
            
            log::debug!("Length of region list content: {}", content.len());
            let tail = if content.len() > 100 { content.len() - 100 } else { 0 };
            log::debug!("Tail of content: {}", &content[tail..]);
            let dom = Dom::parse(&content)?;
            let next_data = find_element_by_id(&dom.children, NEXT_DATA);
            //////log::debug!("Found element: {:?}", next_data);    // ***TEMP***
            if let Some(next_data_nodes) = next_data {
                if let Node::Element(elt) = next_data_nodes {
                    if let Node::Text(json_str) = &elt.children[0] {
                        log::debug!("Found JSON: {:.200}", json_str);
                        let parsed_json = json::parse(json_str)?;
                        //   "props": {
                        //      "pageProps": {
                        //          "regionListStaticProps": {
                        //              "data": [
                        let regions = &parsed_json["props"]["pageProps"]["regionListStaticProps"]["data"];
                        Ok(regions.clone())                        
                    } else {
                        Err(anyhow!("Did not find JSON in NEXT_DATA: {:?}", next_data))
                    }
                } else {
                    Err(anyhow!("Did not find element in NEXT_DATA: {:?}", next_data))
                }
            } else {     
                Err(anyhow!("Did not find {} ID in {}", NEXT_DATA, url))
            }
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
    let regions = fetch_region_list_json(&mut agent).expect("Fetch regions from BonnieBots failed.");
    let region_list = BonnieBotsRegion::from_json(&regions).expect("Conversion from BonnieBots JSON failed.");
    log::debug!("JSON: {} regions.", regions.len());
    for region_item in &region_list[.. 20.min(region_list.len())] {
        log::debug!("    {:?}", region_item)
    }
 }
