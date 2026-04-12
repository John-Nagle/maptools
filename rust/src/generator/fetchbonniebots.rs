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
use anyhow::{anyhow, Error};
use html_parser::{Dom, Node};
use std::io::Read;
use serde::{Deserialize};
use ureq::Agent;
use uuid::Uuid;
use common::{RegionData, HeightField};
use array2d::Array2D;

/// Size of region elev data, SL only.
pub const TERRAIN_DATA_DIM: usize = 256;
const TERRAIN_DATA_SIZE: usize = TERRAIN_DATA_DIM * TERRAIN_DATA_DIM;
/// User agent for talking to asset server
const USER_AGENT: &str = "animats.info impostor asset system";
/// Region size, Second Life only.
pub const SL_REGION_SIZE: u32 = 256;
/// Grid name, the only one supported
pub const SL_GRID: &str = "agni";

/// Convert flattened elevs array to 2D array.
/// SL only, 256x256 samples.
/// ***NOT SURE ABOUT AXIS ORDER*** CHECK
fn convert_to_2d_array(heights: Vec<f32>) -> Array2D<f32> {
    Array2D::from_iter_row_major(heights.into_iter(), TERRAIN_DATA_DIM, TERRAIN_DATA_DIM).unwrap()
}
/// Fetch elevation data from Bonniebots. 256x256.
pub fn fetch_elevs(
    agent: &mut Agent,
    region_num_x: u32,
    region_num_y: u32,
) -> Result<Option<Array2D<f32>>, Error> {
    // Build URL
    let url = format!(
        "https://www.bonniebots.com/static-api/terrain/{}-{}.bin",
        region_num_x, region_num_y
    );
    match agent.get(&url).call() {
        Ok(mut response) => {
            let content = response.body_mut().read_to_vec()?;
            log::debug!("Length of content: {}", content.len());
            const FLOAT_SIZE: usize = 4; // this is a constant somewhere
            if content.len() != TERRAIN_DATA_SIZE * FLOAT_SIZE {
                Err(anyhow!(
                    "Elevation data has wrong size: {}. Expected {}. URL: {}",
                    content.len(),
                    TERRAIN_DATA_SIZE * FLOAT_SIZE,
                    url
                ))
            } else {
                let elevs: Vec<_> = content
                    .chunks(FLOAT_SIZE)
                    .map(|c: &[u8]| f32::from_le_bytes(c.try_into().unwrap()))
                    .collect();
                log::info!("HTTP success reading elev data from {}", url);
                log::debug!("Elevs: {:?}", &elevs[0..4]); // ***TEMP***
                Ok(Some(convert_to_2d_array(elevs))) // ***TEMP***
            }
        }
        Err(ureq::Error::StatusCode(code)) => {
            // the server returned an unexpected status
            log::error!("HTTP fail, code {}, reading elev data from {}", code, url);
            match code {
                404 => Ok(None),
                _ => Err(anyhow!("HTTP error {} reading elev data from {}", code, url)),
            }
        }
        Err(e) => Err(e.into()),
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
                        return Some(node);
                    }
                }
                //  Recurse
                if let Some(found) = find_element_by_id(&e.children, id_key) {
                    return Some(found);
                }
            }
            Node::Text(_s) => {}
            Node::Comment(_c) => {}
        }
    }
    //  No find
    None
}

/// More detailed info for a BonnieBots region.
/// Only the fields we are interested in.
#[derive(Deserialize, Debug, Clone)]
pub struct BonnieBotsRegion {
    /// Region name
    region_name: String,
    /// UUID of region
    region_map_image: Uuid,
    ///  Region loc, in region counts, not meters.
    region_x: u32,
    ///  Region loc, in region counts, not meters.
    region_y: u32,
    /// Water height in mm.
    water_height_mm: Option<u32>,
}

impl BonnieBotsRegion {
    /// Fetch for a given region
    pub fn fetch(agent: &mut Agent, region_x: u32, region_y: u32) -> Result<Option<Self>, Error> {
        let url = format!(
            "https://www.bonniebots.com/static-api/regions/{}/{}/index.json",
            region_x, region_y,
        );
         match agent
            .get(&url)
            //////.header("user-agent", "curl/7.81.0")
            .call()
        {
            Ok(mut response) => {
                let json_str =  response.body_mut().read_to_string()?;
                log::info!("BB region JSON: {:?}", json_str);
                let region: Self = serde_json::from_str(&json_str)?;
                log::info!("BB region info: {:?}", region);
                Ok(Some(region))
             }
            Err(ureq::Error::StatusCode(code)) => {
                // the server returned an unexpected status
                log::error!("HTTP fail, code {}, reading region info from {}", code, url);
                match code {
                    //  No find is not an error, just a hole in the map.
                    404 => Ok(None),
                    _ => Err(anyhow!("HTTP error {} reading region data from {}", code, url)),
                }
            }
            Err(e) => Err(e.into()),
        }
    }

}

/// Info BonnieBots can provide for one region, from the region list of all regions.
/// This is short and basic.
pub struct BonnieBotsBasicRegion {
}

impl BonnieBotsBasicRegion {
    
    /// Into RegionData
    pub fn new_region_data(
        region_name: &json::JsonValue,
        region_x: &json::JsonValue,
        region_y: &json::JsonValue,
        ) -> Result<RegionData, Error> {
            let name = region_name.as_str().ok_or_else(|| anyhow!("No region name"))?.trim().to_string();
            let region_loc_x = region_x.as_u32().ok_or_else(|| anyhow!("No region size X"))? * SL_REGION_SIZE;
            let region_loc_y = region_y.as_u32().ok_or_else(|| anyhow!("No region size X"))? * SL_REGION_SIZE;
            Ok(RegionData {
                name,
                region_loc_x,
                region_loc_y,
                region_size_x: SL_REGION_SIZE,
                region_size_y: SL_REGION_SIZE,
                lod: 0,
                grid: SL_GRID.to_string()
            })
    }

    ///   Build from BonnieBots JSON data
    pub fn from_json(regions: &json::JsonValue) -> Result<Vec<RegionData>, Error> {
        let mut region_records = Vec::new();
        if let json::JsonValue::Array(regions_array) = regions {
            for region in regions_array {
                //  Region data is an array
                let region_name = &region[0];
                let region_x = &region[6];
                let region_y = &region[7];
                region_records.push(
                    Self::new_region_data(region_name, region_x, region_y)?
                );
            }
        } else {
            return Err(anyhow!(
                "Did not find array in regions JSON from Bonniebots."
            ));
        }
        Ok(region_records)
    }

    /// Fetch region list from BonnieBots.
    /// This assumes a very specific page layout at BonnieBots.
    /// An API would be better.
    pub fn fetch_region_list_json(agent: &mut Agent) -> Result<json::JsonValue, Error> {
        const BONNIE_BOTS_BASIC_REGION_URL: &str = "https://www.bonniebots.com/regions";
        let url = BONNIE_BOTS_BASIC_REGION_URL;
        const NEXT_DATA: &str = "__NEXT_DATA__";
        match agent
            .get(url)
            //////.header("user-agent", "curl/7.81.0")
            .call()
        {
            Ok(mut response) => {
                //////let content = response.body_mut().read_to_string()?;
                let mut reader = response.body_mut().as_reader();
                let mut body = Vec::new();
                reader.read_to_end(&mut body)?; // Ensures all data is read
                let content = String::from_utf8(body)?;

                log::debug!("Length of region list content: {}", content.len());
                let tail = if content.len() > 100 {
                    content.len() - 100
                } else {
                    0
                };
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
                            let regions =
                                &parsed_json["props"]["pageProps"]["regionListStaticProps"]["data"];
                            Ok(regions.clone())
                        } else {
                            Err(anyhow!("Did not find JSON in NEXT_DATA: {:?}", next_data))
                        }
                    } else {
                        Err(anyhow!(
                            "Did not find element in NEXT_DATA: {:?}",
                            next_data
                        ))
                    }
                } else {
                    Err(anyhow!("Did not find {} ID in {}", NEXT_DATA, url))
                }
            }
            Err(ureq::Error::StatusCode(code)) => {
                // the server returned an unexpected status
                Err(anyhow!("HTTP error {} reading {}", code, url))
            }
            Err(e) => Err(e.into()),
        }
    }
}
/// Fetch height map for one region.
/// Location is in SL region counts, not meters.
pub fn fetch_height_field(agent: &mut Agent, region_x: u32, region_y: u32) -> Result<Option<HeightField>, Error> {
    let elevs_opt = fetch_elevs(agent, region_x, region_y)?;
    let region_data_opt = BonnieBotsRegion::fetch(agent, region_x, region_y)?;
    if region_data_opt.is_none() || region_data_opt.as_ref().unwrap().water_height_mm.is_none()  {
        log::error!("BonnieBots region data is missing for region ({}, {})", region_x, region_y);
    }
    if elevs_opt.is_none() {
        log::error!("BonnieBots elevation data is missing for region ({}, {})", region_x, region_y);
    }
    if let Some(elevs) = elevs_opt && let Some(region_data) = region_data_opt && let Some(water_height_mm) = region_data.water_height_mm {
        Ok(Some(HeightField::new(elevs, SL_REGION_SIZE, SL_REGION_SIZE, (water_height_mm as f32) / 1000.0)))
    } else {
        //  No data, but not an abort condition
        Ok(None)
    }
}

#[test]
/// Fetch all elevations for a single region.
fn test_fetchelevs() {
    //  All errors to console
    use common::test_logger;
    test_logger();
    //  HTTP Agent
    let config = Agent::config_builder().build();
    let mut agent: Agent = config.into();
    let elevs =
        fetch_elevs(&mut agent, 1000, 1000).expect("Fetch elevations from BonnieBots failed.");
}

#[test]
/// Fetch all regions
fn test_fetchregions() {
    //  All errors to console
    use common::test_logger;
    use crate::VizGroups;
    test_logger();
    //  HTTP Agent
    let config = Agent::config_builder().build();
    let mut agent: Agent = config.into();
    let regions = BonnieBotsBasicRegion::fetch_region_list_json(&mut agent)
        .expect("Fetch regions from BonnieBots failed.");
    let mut region_list =
        BonnieBotsBasicRegion::from_json(&regions).expect("Conversion from BonnieBots JSON failed.");
    log::debug!("JSON: {} regions.", regions.len());
    //  Get the visgroups data.
    log::info!("Vizgroups build start"); // ***TEMP***
    //  Sort by region_data by x, y, grid
    region_list.sort_by(|a, b| (&a.grid, a.region_loc_x, a.region_loc_y).cmp(&(&b.grid, b.region_loc_x, b.region_loc_y)));
    let mut viz_groups = VizGroups::new(false);
/*
    //  "grids" is a misnomer here. It's a vec of completed groups.
    let mut grids = Vec::new();
    for region_data in &region_list {
        if let Some(completed_groups) = viz_groups.add_region_data(region_data.clone()) {
            log::debug!("PUSH completed group");
            grids.push(completed_groups);
        }
    }
    grids.push(viz_groups.end_grid());
*/    
    for item in region_list {
        let grid_break = viz_groups.add_region_data(item);
        //  This example is all one grid, so there's no control break.
        assert_eq!(grid_break, None);
    }
    let mut results = viz_groups.end_grid();
    //  Display results
    log::info!("Result: Viz groups: {}", results.len());
    results.sort_by(|a, b| b.len().partial_cmp(&a.len()).unwrap());
    for viz_group in results.iter() {
        if viz_group.len() <= 1 {
            continue
        }
        log::info!("Reachable group, {} regions, first region: {:?}", viz_group.len(), viz_group[0].name);
        for n in 0..viz_group.len().min(30) {
            log::debug!("  {}", viz_group[n]);
        }
    }
/*
    
    
    log::info!("{} regions, {} viz groups.", region_list.len(), grids.len());
    //  Sort by length of vizgroup
    for viz_group_set in &mut grids {
        viz_group_set.sort_by(|a, b| b.len().partial_cmp(&a.len()).unwrap());
        log::info!("Viz group, {} regions: {:?}", viz_group_set.len(), "reg");
    }
*/
    log::info!("Vizgroups build end"); 
/*    
    //  Dump some region elevs.
    for region_item in &region_list[..5.min(region_list.len())] {
        log::debug!("    {:?}", region_item);
        let elevs = fetch_elevs(&mut agent, region_item.region_loc_x / SL_REGION_SIZE, region_item.region_loc_y / SL_REGION_SIZE)
            .expect("Fetch elevations from BonnieBots failed.");
        if elevs.is_none() {
            log::error!("No region data avaiable reading elev data from {:?}", region_item);
        }
        let region_info = BonnieBotsRegion::fetch(&mut agent, region_item).expect("Region data fetch failed");
        log::debug!("Region info: {:?}", region_info);
    }
*/
}
