//! Generate Second Life / Open Simulator terrain objects as files to be uploaded.
//! Part of the Animats impostor system
//!
//!
//! In the previous step, a bot, or a large number of users, visited all regions
//! while carrying a script which talks to the terrain uploader. That data
//! should now be in the terrain database, in the raw_terrain_heights table.
//!
//! This program processes that data and generates images and meshes to
//! be uploaded. These go into a local directory.
//! This runs as a command line program, or perhaps a cron job.
//!
//!     License: LGPL.
//!     Animats
//!     August, 2025.
//
#![forbid(unsafe_code)]
mod sculptmaker;
mod regionorder;
mod vizgroup;
use anyhow::{anyhow, Error};
use common::{HeightField, RegionImpostorFaceData};
use envie::Envie;
use getopts::Options;
use log::LevelFilter;
use mysql::prelude::{Queryable};
use mysql::{params, PooledConn};
use mysql::{Pool};
use std::collections::HashMap;
use std::path::PathBuf;
use vizgroup::{CompletedGroups, RegionData, VizGroups};
use sculptmaker::{TerrainSculpt, TerrainSculptTexture};
use regionorder::{TileLods, homogeneous_group_size};
use ureq::{Agent};


/// MySQL Credentials for uploading.
/// This filename will be searched for in parent directories,
/// so it can be placed above the web root, where the web server can't see it.
/// The upload credentials file must contain
///
///     DB_USER = username
///     DB_PASS = databasepassword
///     DB_HOST = hostname
///     DB_PORT = portnumber (optional, defaults to 3306)
///     DB_NAME = databasename
///
/// The table name is hard-coded.
///
/// Environment variables for obtaining owner info.
/// ***ADD VALUES FOR OPEN SIMULATOR***
const _OWNER_NAME: &str = "HTTP_X_SECONDLIFE_OWNER_NAME";
/// Size of output terrain sculpt textures, pixels.
const TERRAIN_SCULPT_TEXTURE_SIZE: u32 = 256;
/// User agent for talking to asset server
const TERRAIN_GENERATOR_USER_AGENT: &str = "animats.info impostor asset system";

/// Debug logging
fn logger() {
    //  Local log file.
    const LOG_FILE_NAME: &str = "logs/generatelog.txt";
    let _ = simplelog::CombinedLogger::init(vec![simplelog::WriteLogger::new(
        LevelFilter::Debug,
        simplelog::Config::default(),
        std::fs::File::create(LOG_FILE_NAME).expect("Unable to create log file"),
    )]);
    log::warn!("Logging to {:?}", LOG_FILE_NAME); // where the log is going
}

/// Type of UUID
pub enum UuidUsage {
    Texture,
    Sculpt,
    Mesh
}

   
/// Hash info for all components of one tile.
/// Used for unduplication.
/// Hashes here are 16 hex characters.
#[derive(Debug, Clone)]
struct TileHashes {
    /// Sculpt UUID
    sculpt_uuid: Option<String>,
    /// Sculpt hash
    sculpt_hash: Option<String>,
    /// Mesh UUID
    mesh_uuid: Option<String>,
    /// Mesh hash
    mesh_hash: Option<String>,
    /// Hashes of all the textures are included in face data
    /// For meshes, there can be up to 8. Sculpts only have one.
    face_data: Vec<RegionImpostorFaceData>,
}

impl TileHashes {
    /// Is this terrain model known?
    pub fn is_model_known(&self, terrain_generator: &TerrainGenerator) -> Result<bool, Error> {
        todo!();
    }
    
    /// Is this texture known?
    pub fn is_texture_known(&self, terrain_generator: &TerrainGenerator, texture_ix: usize) -> Result<bool, Error> {
        if texture_ix < self.face_data.len() {
            let face_item = &self.face_data[texture_ix];
            todo!();    // ***NEED HASH DATA WHICH IS NOT IN FACE DATA YET***
            
        } else {
            //  Not known
            Ok(false)
        }
    }
    
    /// Does this UUID exist on the asset server?
    /// Returns true if there is no URL prefix available, indicating we don't know how to query the asset server.
    fn test_uuid_valid(terrain_generator: &TerrainGenerator, uuid: uuid::Uuid, _uuid_usage: UuidUsage) -> Result<bool, Error> {
        let url_prefix = if let Some(url_prefix) = &terrain_generator.url_prefix_opt {
            url_prefix
        } else {
            return Ok(true)
        };
        let url = url_prefix.to_string() + &uuid.to_string();
        let mut resp = terrain_generator.agent.head(&url)
            .header("Content-Type", "any") // 
            .call();
        log::debug!("Test UUID valid. {} -> {:?}", url, resp);
        match resp {
            Ok(_) => Ok(true),
            Err(ureq::Error::StatusCode(code)) => {
                match code {
                    404 => Ok(false),
                    _ => Err(anyhow!("HTTP Error {} checking url {}", code, url))
                }
            }
            Err(e) => Err(anyhow!("Error {:?} checking url {}", e, url))
        }
    }
}

/// Key for cache of region info for all LODs.
/// All cache items must be from the same grid.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RegionLodKey {
    /// Location in world of region (meters)
    region_coords_x: u32,
    /// Location in world of region (meters)
    region_coords_y: u32, 
    /// Level of detail.
    lod: u8,
}

/// Height field cache.
/// Height fields for LOD 0 come from the database.
/// Height fields for lower LODs are computed by
/// combining the height fields of four tiles.
///
/// Because of the order in which regionorder
/// returns the desired regions and LODs, each
/// heigh field is only needed once. So 
/// obtaining a height field consumes it.
/// This bounds the memory required.
#[derive(Debug)]
struct HeightFieldCache {
    /// The cache
    cache: HashMap<RegionLodKey, HeightField>,
}

impl HeightFieldCache {
    /// Usual new
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }
    
    /// Insert.
    /// Panics on duplicate insert
    fn insert(&mut self, key: RegionLodKey, height_map: HeightField) {
        if self.cache.insert(key.clone(), height_map).is_some() {
            panic!("Duplicate insert into height field cache: {:?}", key);
        }
    }
    
    /// Destructive remove 
    fn take(&mut self, key: &RegionLodKey) -> Option<HeightField> {
        self.cache.remove(key)
    }
}

/// The terrain object generator
struct TerrainGenerator {
    /// SQL connection
    conn: PooledConn,
    /// Network connection pool
    agent: Agent,
    /// Output directory
    outdir: PathBuf,
    /// Asset server URL prefix
    url_prefix_opt: Option<String>,
    /// Are regions with only corners touching adjacent?
    /// Set to true for Open Simulator grids
    corners_touch_connects: bool,
    /// Generate glTF mesh if on.
    generate_mesh: bool,
    /// The height field cache
    height_field_cache: HeightFieldCache,
}

impl TerrainGenerator {
    /// Usual new.
    pub fn new(
        conn: PooledConn,
        outdir: PathBuf,
        url_prefix_opt: Option<String>,
        corners_touch_connects: bool,
        generate_mesh: bool,
    ) -> Self {
        //  HTTP connection pool, used to validate UUIDs against asset server.
        let config = Agent::config_builder()
            .user_agent(TERRAIN_GENERATOR_USER_AGENT)
            .build();
        let agent: Agent = config.into();
        Self {
            conn,
            agent,
            outdir,
            url_prefix_opt,
            corners_touch_connects,
            generate_mesh,
            height_field_cache: HeightFieldCache::new(),
        }
    }

    /// Build visibility group info from database
    pub fn transitive_closure(&mut self, grid: &str) -> Result<Vec<CompletedGroups>, Error> {
        let mut vizgroups = VizGroups::new(self.corners_touch_connects);
        let mut grids = Vec::new();
        log::info!("Build start"); // ***TEMP***
                                   //  The loop here is sequential data processing with control breaks when an index field changes.
        const SQL_SELECT: &str = r"SELECT grid, region_coords_x, region_coords_y, size_x, size_y, name FROM raw_terrain_heights WHERE LOWER(grid) = :grid ORDER BY grid, region_coords_x, region_coords_y ";
        let _all_regions = self.conn.exec_map(
            SQL_SELECT,
            params! { grid },
            |(grid, region_coords_x, region_coords_y, size_x, size_y, name)| {
                let region_data = RegionData {
                    grid,
                    region_coords_x,
                    region_coords_y,
                    size_x,
                    size_y,
                    name,
                    lod: 0,
                };
                if let Some(completed_groups) = vizgroups.add_region_data(region_data) {
                    grids.push(completed_groups);
                }
            },
        )?;
        grids.push(vizgroups.end_grid());
        Ok(grids)
    }

    /// Get elevation data for one region.
    pub fn get_height_field_one_region(
        &mut self,
        grid: String,
        region_coords_x: u32,
        region_coords_y: u32,
    ) -> Result<HeightField, Error> {
        const SQL_SELECT: &str = r"SELECT size_x, size_y, samples_x, samples_y, scale, offset, elevs, name, water_level
                FROM raw_terrain_heights
                WHERE LOWER(grid) = :grid AND region_coords_x = :region_coords_x AND region_coords_y = :region_coords_y";
        let grid_for_msg = grid.clone();
        let mut height_fields = self.conn.exec_map(
            SQL_SELECT,
            params! { grid, region_coords_x, region_coords_y },
            |(size_x, size_y, samples_x, samples_y, scale, offset, elevs, name, water_level)| {
                let _name_v: String = name;
                let _water_level_v: f32 = water_level;
                let height_field = HeightField::new_from_elevs_blob(
                    &elevs, samples_x, samples_y, size_x, size_y, scale, offset, water_level,
                );
                height_field
            },
        )?;
        if height_fields.is_empty() {
            return Err(anyhow!(
                "No raw terrain data for region at ({},{}) on \"{}\"",
                region_coords_x,
                region_coords_y,
                grid_for_msg
            ));
        }

        if height_fields.len() > 1 {
            //  Duplicate data - warning
            //  SQL indices should make this impossible.
            log::error!(
                "More than one region data set for region at ({},{}) on \"{}\"",
                region_coords_x,
                region_coords_y,
                grid_for_msg
            );
        }
        let height_field = height_fields.pop().unwrap()?;
        //  Cache for later generation of lower LODs
        let key = RegionLodKey { lod: 0, region_coords_x, region_coords_y };
        self.height_field_cache.insert(key, height_field.clone());
        Ok(height_field)
    }
    
    /// Get height field for multiple regions.
    /// We fetch four regions and merge them.
    pub fn get_height_field_multi_region(
        &mut self,
        _grid: String,
        region_coords_x: u32,
        region_coords_y: u32,
        region_size: (u32, u32),
        lod: u8) -> Result<HeightField, Error> {
        //  Not for LOD 0. We can't build that from other LODs.
        assert!(lod > 0);
        //  Get a relevant region, or None if it's all water.
        //  May need more checking for missing regions.
        let mut take = |lod, dx, dy| {
            let key = RegionLodKey { lod, region_coords_x: region_coords_x + dx, region_coords_y: region_coords_y + dy };
            log::debug!("Multi region height field needed for LOD {}: {:?}", key.lod, (key.region_coords_x, key.region_coords_y));  // ***TEMP***
            self.height_field_cache.take(&key)
        };
        //  Get the four height fields.
        //  Region size here is the full sized impostor, so we have to divide by 2 to get the size of the 4 squares that make it up.
        let height_fields = [
            take(lod - 1, 0, 0),            
            take(lod - 1, region_size.0 / 2, 0),
            take(lod - 1, 0, region_size.1 / 2),
            take(lod - 1, region_size.0 / 2, region_size.1 / 2)
        ];
        //  Generate combined height field;
        let height_field = HeightField::halve(&HeightField::combine(height_fields)?);
        let key = RegionLodKey { lod , region_coords_x, region_coords_y };
        self.height_field_cache.insert(key, height_field.clone());
        Ok(height_field)
    }
    
    /// Encoded name for impostor asset file.
    /// The name contains all the info we need to generate the impostor.
    /// Format: RS_x_y_sx_sy_sz_offset_lod_waterlevel_name
    fn impostor_name(
        prefix: &str,
        region: &RegionData,
        height_field: &HeightField,
        lod: u8,
        hash: u64,
    ) -> Result<String, Error> {
        let x = region.region_coords_x;
        let y = region.region_coords_y;
        let (scale, offset) = height_field.get_scale_offset()?;
        let sx = region.size_x;
        let sy = region.size_y;
        let sz = scale;
        let water_level = height_field.water_level;
        Ok(format!("{}_{}_{}_{}_{}_{:.2}_{:.2}_{}_{:.2}_0x{:016x}", prefix, x, y, sx, sy, sz, offset, lod, water_level, hash))
    }
    
    /// Get all the hash values for one tile.
    /// This is used to see if the tile has already been uploaded.
    fn get_hashes_one_tile(&mut self, grid: &str, region_loc_x: u32, region_loc_y: u32, impostor_lod: u8) -> Result<Option<TileHashes>, Error> {
        const SQL_SELECT: &str = r"SELECT sculpt_uuid, sculpt_hash, mesh_uuid, mesh_hash, faces_json
            FROM region_impostors
            WHERE LOWER(grid) = :grid AND region_loc_x = :region_loc_x AND region_loc_y = :region_loc_y AND impostor_lod = :impostor_lod";
        let tile_hashes = self.conn.exec_map(
            SQL_SELECT,
            params! { grid, region_loc_x, region_loc_y, impostor_lod },
            |(sculpt_uuid, sculpt_hash, mesh_uuid, mesh_hash, faces_json)| {
                let faces_json: String = faces_json;    // type inference needs a hint here
                let face_data: Vec<RegionImpostorFaceData> = match serde_json::from_str(&faces_json) {
                    Ok(v) => v,
                    Err(e) => {
                        log::error!("Invalid stored JSON for tile at {} ({}, {}) lod {}: {:?}",
                            grid, region_loc_x, region_loc_y, impostor_lod, e);
                        //  Return empty vector on error
                        vec![]
                    }
                };
                TileHashes {
                    sculpt_uuid,
                    sculpt_hash,
                    mesh_uuid,
                    mesh_hash,
                    face_data,
                }
            },
        )?;
        //  There should be zero or one hits in the database.
        //  More than one indicates a bad SQL table configuration.
        match tile_hashes.len() {
            0 => Ok(None),
            1 => Ok(Some(tile_hashes[0].clone())),
            _ => Err(anyhow!("Duplicate entry for tile at  {} ({}, {}) lod {}",
                  grid, region_loc_x, region_loc_y, impostor_lod)),
        }
    }
    
    /// Build the impostor
    pub fn build_impostor(
        &mut self,
        region: &RegionData,
        height_field: &HeightField,
    ) -> Result<(), Error> {
        let hash_info_opt = self. get_hashes_one_tile(&region.grid, region.region_coords_x, region.region_coords_y, region.lod)?;
        log::debug!("Hash info: {:?}", hash_info_opt);
        if self.generate_mesh {
            self.build_impostor_mesh(
                region,
                height_field,
            )
        } else {
            self.build_impostor_sculpt(
                region,
                height_field,
            )
        }
    }

    /// Build the impostor as a sculpt.
    pub fn build_impostor_sculpt(
        &mut self,
        region: &RegionData,
        height_field: &HeightField,
    ) -> Result<(), Error> {
        const IMPOSTOR_SCULPT_PREFIX: &str = "RS";
        const IMPOSTOR_TERRAIN_PREFIX: &str = "RT0";
        let lod = region.lod;
        log::info!("Generating sculpt for \"{}\": {}", region.name, height_field);
        // TerrainSculpt was translated from Python with an LLM. NEEDS WORK
        let sculpt_impostor_name = region.name.clone(); // ***TEMP***
        let mut terrain_sculpt = TerrainSculpt::new(&sculpt_impostor_name);
        let (scale, offset, elevs) = height_field.into_sculpt_array()?;
        terrain_sculpt.setelevs(elevs, scale as f64, offset as f64);
        terrain_sculpt.makeimage();
        let hash = terrain_sculpt.get_hash()?;
        let sculpt_name = Self::impostor_name(IMPOSTOR_SCULPT_PREFIX, region, height_field, lod, hash)?;
        let sculpt_image = terrain_sculpt.image.unwrap();
        let mut sculpt_image_path = self.outdir.clone();
        sculpt_image_path.push(sculpt_name.to_owned() + ".png");
        
        log::info!("Generating texture for  \"{}\"", sculpt_image_path.display());
        let mut terrain_image = TerrainSculptTexture::new(region.region_coords_x, region.region_coords_y, lod, &region.name);
        terrain_image.makeimage(TERRAIN_SCULPT_TEXTURE_SIZE)?;
        let hash = terrain_image.get_hash()?;
        let terrain_image_name = Self::impostor_name(IMPOSTOR_TERRAIN_PREFIX, region, height_field, lod, hash)?;
        
        let mut terrain_image_path = self.outdir.clone();
        terrain_image_path.push(terrain_image_name.to_owned() + ".png");
        let terrain_image = terrain_image.image.unwrap();
        //  Did both sculpt and its one texture. Now OK to write files
        sculpt_image.save(sculpt_image_path)?;
        log::info!("Sculpt image saved: \"{}\"", terrain_image_path.display());    
        terrain_image.save(&terrain_image_path)?;
        log::info!("Sculpt terrain image saved: \"{}\"", terrain_image_path.display());        
        Ok(())
    }

    /// Build the impostor as a glTF mesh.
    pub fn build_impostor_mesh(
        &mut self,
        _region: &RegionData,
        _height_field: &HeightField,
    ) -> Result<(), Error> {
        todo!("glTF mesh generation is not implemented yet");
    }
    
    /// Build an impostor for LOD N.
    fn build_impostor_for_lod(&mut self, region: &RegionData, _region_size_opt: Option<(u32, u32)>) -> Result<(), Error> {
        log::info!("Region \"{}\", LOD {} starting.", region.name, region.lod);
        let height_field = if region.lod == 0 {
            self.get_height_field_one_region(
                region.grid.clone(),
                region.region_coords_x,
                region.region_coords_y,
            )?
        } else {
            self.get_height_field_multi_region(
                region.grid.clone(),
                region.region_coords_x,
                region.region_coords_y,               
                (region.size_x, region.size_y),
                region.lod,
            )?
        };
        self.build_impostor(
            region,
            &height_field,
        )?;
        log::info!("Region \"{}\", LOD {} built.", region.name, region.lod);
        Ok(())
    }
    
    /// Process group, multi-LOD version
    fn process_group(&mut self, group: Vec<RegionData>) -> Result<(), Error> {
        log::info!("Group: {} entries.", group.len());
        let region_size_opt = homogeneous_group_size(&group);
        if region_size_opt.is_some() && group.len() > 1 {
            //  Do the LOD thing.
            for region in TileLods::new(group) {
                self.build_impostor_for_lod(&region, region_size_opt)?;
            }
        } else {
            //  LOD 0 only.
            for region in group {
                self.build_impostor_for_lod(&region, None)?;
            }
        }
        Ok(())
    }

    /// Process one grid, with multiple visibilty groups
    pub fn process_grid(&mut self, mut completed_groups: CompletedGroups) -> Result<(), Error> {
        completed_groups.sort_by(|a, b| b.len().partial_cmp(&a.len()).unwrap());
        for group in completed_groups {
            self.process_group(group)?;
        }
        Ok(())
    }
}

/// Actually do the work
fn run(pool: Pool, outdir: PathBuf, grid: String, url_prefix_opt: Option<String>, generate_mesh: bool) -> Result<(), Error> {
    let corners_touch_connects = false; // for now, SL only.
    let conn = pool.get_conn()?;
    let mut terrain_generator =
        TerrainGenerator::new(conn, outdir, url_prefix_opt, generate_mesh, corners_touch_connects);
    let mut grids = terrain_generator.transitive_closure(&grid)?;
    if grids.is_empty() {
        return Err(anyhow!("Grid \"{}\" not found.", grid));
    }

    if grids.len() != 1 {
        return Err(anyhow!(
            "More than one grid found but SQL should return only one grid."
        ));
    }
    let grid_entry = grids.pop().unwrap(); // get the one grid
    terrain_generator.process_grid(grid_entry)?;
    Ok(())
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}

/// Set up options, credentials, and database connection.
fn setup() -> Result<(Pool, PathBuf, String, Option<String>, bool), Error> {
    //  Usual options processing
    let args: Vec<String> = std::env::args().collect();
    let program = args[0].clone();
    //  The options
    let mut opts = Options::new();
    opts.optopt("o", "outdir", "Set output directory name.", "NAME");
    opts.optopt(
        "c",
        "credentials",
        "Get database credentials from this file.",
        "NAME",
    );
    opts.optflag("m", "mesh", "Generate glTF mesh, not sculpt image");
    opts.optopt("g", "grid", "Only output for this grid", "NAME");
    opts.optopt("p", "prefix", "Asset server URL prefix for validating assets", "NAME");
    opts.optflag("h", "help", "Print this help menu.");
    opts.optflag("v", "verbose", "Verbose mode.");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            panic!("{}", f.to_string());
        }
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        panic!("Help requested, will not run.");
    }
    let outdir = matches.opt_str("o");
    let credsfile = matches.opt_str("c");
    let verbose = matches.opt_present("v");
    let grid = matches.opt_str("g");
    let url_prefix_opt = matches.opt_str("p");
    let generate_mesh = matches.opt_present("m");
    if outdir.is_none() || credsfile.is_none() || grid.is_none() {
        print_usage(&program, opts);
        return Err(anyhow!("Required command line options missing"));
    }
    let credsfile = credsfile.unwrap();
    let outdir = PathBuf::from(&outdir.unwrap());
    let grid = grid.unwrap().trim().to_lowercase();
    // Create the output directory, empty.
    std::fs::create_dir_all(&outdir)?;
    // Connect to the database
    let creds = match Envie::load_with_path(&credsfile) {
        Ok(creds) => creds,
        Err(e) => {
            //  Envie returns a string and we need an Error
            return Err(anyhow!(
                "Unable to open credentials file \"{}\": {:?}",
                credsfile,
                e
            ));
        }
    };
    //  Optional MySQL port number
    let portnum = if let Some(port) = creds.get("DB_PORT") {
        port.parse::<u16>()?
    } else {
        //  Use MySQL default
        3306
    };
    let opts = mysql::OptsBuilder::new()
        //  Dreamhost is still using old authentication
        .secure_auth(false)
        .ip_or_hostname(creds.get("DB_HOST"))
        .tcp_port(portnum)
        .user(creds.get("DB_USER"))
        .pass(creds.get("DB_PASS"))
        .db_name(creds.get("DB_NAME"));
    drop(creds);
    log::info!("Opts: {:?}", opts);
    let pool = Pool::new(opts)?;
    if verbose {
        println!("Connected to database.");
    }
    log::info!("Connected to database.");
    //  Setup complete. Return what's needed to run.
    Ok((pool, outdir, grid, url_prefix_opt, generate_mesh))
}

/// Main program.
/// Setup, then run.
fn main() {
    logger();
    match setup() {
        Ok((pool, outdir, grid, url_prefix_opt, mesh)) => match run(pool, outdir, grid, url_prefix_opt, mesh) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed: {:?}", e);
            }
        },
        Err(e) => {
            panic!("Unable to start: {:?}", e);
        }
    };
}

