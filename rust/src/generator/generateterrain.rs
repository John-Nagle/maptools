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
use common::{RegionData, HeightField, RegionImpostorFaceData, InitialImpostors, TileType};
use envie::Envie;
use getopts::Options;
use log::LevelFilter;
use mysql::prelude::{Queryable};
use mysql::{params, PooledConn};
use mysql::{Pool};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use vizgroup::{CompletedGroups, VizGroups};
use sculptmaker::{TerrainSculpt, TerrainSculptTexture};
use regionorder::{TileLods, homogeneous_group_size};
use common::{hash_to_hex, AssetUpload, TileAssetType};
use ureq::{Agent};
use chrono::Utc;

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
/// Files per directory. A convenient size. Per prim limit is supposedly 10,000, but we might hit some viewer limit for cut and paste.
const FILES_PER_DIRECTORY: usize = 1000;

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


/// Key for cache of region info for all LODs.
/// All cache items must be from the same grid.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RegionLodKey {
    /// Location in world of region (meters)
    region_loc_x: u32,
    /// Location in world of region (meters)
    region_loc_y: u32, 
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

/// Statistics for terrain generator
struct TerrainGeneratorStats {
    /// Generated, must upload to SL/OS.
    assets_generated: usize,
    /// Reused, nothing to upload to SL/OS
    assets_reused: usize,
}

impl TerrainGeneratorStats {
    /// Usual new
    fn new() -> Self {
        Self {
            assets_generated: 0,
            assets_reused: 0,
        }
    }
}

impl std::fmt::Display for TerrainGeneratorStats {
    // Implement `fmt::Display` for the struct
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "Assets generated: {}\nAssets reused:   {}", self.assets_generated, self.assets_reused)
    }
}

/// Organizes files into multiple directories, for convenient uploading into SL/OS
struct FolderGenerator {
    /// Base directory
    base_dir: PathBuf,
    /// Files per directory
    files_per_directory: usize,
    /// Files generated
    file_count: usize,
}

impl FolderGenerator {
    /// Usual new
    fn new(base_dir: &Path, files_per_directory: usize) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
            files_per_directory,
            file_count: 0,
        }
    }
    /// Returns desired directory name. Creates directory if needed
    /// Directory names are simply prefix/Rnn
    fn next_path(&mut self) -> Result<PathBuf, Error> {
        //  Create a subdirectory if needed
        let dir_index = self.file_count / self.files_per_directory;
        let dir_name = format!("R{:02}", dir_index);
        let mut path = self.base_dir.clone();
        path.push(dir_name);
        if self.file_count.is_multiple_of(self.files_per_directory) {
            log::info!("Creating output directory {:?}", path);
            std::fs::create_dir_all(&path)?;
        }  
        self.file_count += 1;
        Ok(path)
    }
}

/// The terrain object generator
struct TerrainGenerator {
    /// SQL connection
    conn: PooledConn,
    /// Network connection pool (Future)
    _agent: Agent,
    /// Output directory
    folder_generator: FolderGenerator,
    /// Asset server URL prefix (Future)
    _url_prefix_opt: Option<String>,
    /// Are regions with only corners touching adjacent?
    /// Set to true for Open Simulator grids
    corners_touch_connects: bool,
    /// Generate glTF mesh if on.
    generate_mesh: bool,
    /// The height field cache
    height_field_cache: HeightFieldCache,
    /// Statistics
    stats: TerrainGeneratorStats,
}

impl TerrainGenerator {
    /// Usual new.
    pub fn new(
        conn: PooledConn,
        outdir: PathBuf,
        _url_prefix_opt: Option<String>,
        corners_touch_connects: bool,
        generate_mesh: bool,
    ) -> Self {
        //  HTTP connection pool, used to validate UUIDs against asset server.
        let config = Agent::config_builder()
            .user_agent(TERRAIN_GENERATOR_USER_AGENT)
            .build();
        let _agent: Agent = config.into();
        let folder_generator = FolderGenerator::new(&outdir, FILES_PER_DIRECTORY);
        Self {
            conn,
            _agent,
            folder_generator,
            _url_prefix_opt,
            corners_touch_connects,
            generate_mesh,
            height_field_cache: HeightFieldCache::new(),
            stats: TerrainGeneratorStats::new(),
        }
    }

    /// Build visibility group info from database
    pub fn transitive_closure(&mut self, grid: &str) -> Result<Vec<CompletedGroups>, Error> {
        let mut vizgroups = VizGroups::new(self.corners_touch_connects);
        let mut grids = Vec::new();
        log::info!("Build start"); // ***TEMP***
                                   //  The loop here is sequential data processing with control breaks when an index field changes.
        const SQL_SELECT: &str = r"SELECT grid, region_loc_x, region_loc_y, region_size_x, region_size_y, name FROM raw_terrain_heights WHERE LOWER(grid) = :grid ORDER BY grid, region_loc_x, region_loc_y ";
        let _all_regions = self.conn.exec_map(
            SQL_SELECT,
            params! { grid },
            |(grid, region_loc_x, region_loc_y, region_size_x, region_size_y, name)| {
                let region_data = RegionData {
                    grid,
                    region_loc_x,
                    region_loc_y,
                    region_size_x,
                    region_size_y,
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
        region_loc_x: u32,
        region_loc_y: u32,
    ) -> Result<HeightField, Error> {
        const SQL_SELECT: &str = r"SELECT region_size_x, region_size_y, samples_x, samples_y, scale, offset, elevs, name, water_level
                FROM raw_terrain_heights
                WHERE LOWER(grid) = :grid AND region_loc_x = :region_loc_x AND region_loc_y = :region_loc_y";
        let grid_for_msg = grid.clone();
        let mut height_fields = self.conn.exec_map(
            SQL_SELECT,
            params! { grid, region_loc_x, region_loc_y },
            |(region_size_x, region_size_y, samples_x, samples_y, scale, offset, elevs, name, water_level)| {
                let _name_v: String = name;
                let _water_level_v: f32 = water_level;
                HeightField::new_from_elevs_blob(
                    &elevs, samples_x, samples_y, region_size_x, region_size_y, scale, offset, water_level,
                )
            },
        )?;
        if height_fields.is_empty() {
            return Err(anyhow!(
                "No raw terrain data for region at ({},{}) on \"{}\"",
                region_loc_x,
                region_loc_y,
                grid_for_msg
            ));
        }

        if height_fields.len() > 1 {
            //  Duplicate data - warning
            //  SQL indices should make this impossible.
            log::error!(
                "More than one region data set for region at ({},{}) on \"{}\"",
                region_loc_x,
                region_loc_y,
                grid_for_msg
            );
        }
        let height_field = height_fields.pop().unwrap()?;
        //  Cache for later generation of lower LODs
        let key = RegionLodKey { lod: 0, region_loc_x, region_loc_y };
        self.height_field_cache.insert(key, height_field.clone());
        Ok(height_field)
    }
    
    /// Get height field for multiple regions.
    /// We fetch four regions and merge them.
    pub fn get_height_field_multi_region(
        &mut self,
        _grid: String,
        region_loc_x: u32,
        region_loc_y: u32,
        region_size: (u32, u32),
        lod: u8) -> Result<HeightField, Error> {
        //  Not for LOD 0. We can't build that from other LODs.
        assert!(lod > 0);
        //  Get a relevant region, or None if it's all water.
        //  May need more checking for missing regions.
        let mut take = |lod, dx, dy| {
            let key = RegionLodKey { lod, region_loc_x: region_loc_x + dx, region_loc_y: region_loc_y + dy };
            log::debug!("Multi region height field needed for LOD {}: {:?}", key.lod, (key.region_loc_x, key.region_loc_y));  // ***TEMP***
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
        let key = RegionLodKey { lod , region_loc_x, region_loc_y };
        self.height_field_cache.insert(key, height_field.clone());
        Ok(height_field)
    }

    
    /// Build the impostor
    pub fn build_impostor(
        &mut self,
        region: &RegionData,
        height_field: &HeightField,
        viz_group_id: u32,
    ) -> Result<(), Error> {
        if self.generate_mesh {
            self.build_impostor_mesh(
                region,
                height_field,
                viz_group_id,
            )
        } else {
            self.build_impostor_sculpt(
                region,
                height_field,
                viz_group_id,
            )
        }
    }

    /// Build the impostor as a sculpt.
    pub fn build_impostor_sculpt(
        &mut self,
        region: &RegionData,
        height_field: &HeightField,
        viz_group_id: u32,
    ) -> Result<(), Error> {
        let lod = region.lod;
        log::info!("Generating sculpt for \"{}\": {}", region.name, height_field);
        // TerrainSculpt was translated from Python with an LLM. NEEDS WORK
        //  Do sculpt
        let mut terrain_sculpt = TerrainSculpt::new(&region.name);
        let (scale, offset, elevs) = height_field.into_sculpt_array()?;
        terrain_sculpt.setelevs(elevs, scale as f64, offset as f64);
        terrain_sculpt.makeimage();
        let sculpt_hash = terrain_sculpt.get_hash()?;
        //  Create an AssetUpload for the one texture.
        let sculpt_asset_upload = AssetUpload::new(TileAssetType::SculptTexture, region, height_field, sculpt_hash)?;    
        let sculpt_uuid_opt = sculpt_asset_upload.get_asset_uuid(&mut self.conn)?;
        if let Some (uuid) = sculpt_uuid_opt {
            log::info!("Sculpt image asset already exists: {} UUID: {:?}", sculpt_asset_upload.asset_name, uuid);
            self.stats.assets_reused += 1;
        } else {
            let sculpt_image = terrain_sculpt.image.unwrap();
            let mut sculpt_image_path = self.folder_generator.next_path()?;
            sculpt_image_path.push(sculpt_asset_upload.asset_name.to_owned() + ".png");
            sculpt_image.save(&sculpt_image_path)?;
            log::info!("Sculpt image file saved: \"{}\"", sculpt_image_path.display());
            println!("Sculpt file: \"{}\"", sculpt_image_path.display());
            //  Timestamp for sculpt is local time, because we built this asset.
            let last_modified = Some(Utc::now());
            let new_tile_created = sculpt_asset_upload.insert_tile_without_uuid(&mut self.conn, last_modified)?;
            if !new_tile_created {
                //  This ought not to happen much, if at all. If it happens generating and uploading were probably out of sequence.
                log::warn!("Duplicate tile sculpt: {:?}", sculpt_asset_upload);
            }
            self.stats.assets_generated += 1;  
        }
        //  Do texture
        log::info!("Generating texture image for  \"{}\"", &region.name);
        let mut terrain_image = TerrainSculptTexture::new(region.region_loc_x, region.region_loc_y, lod, &region.name);
        terrain_image.makeimage(TERRAIN_SCULPT_TEXTURE_SIZE)?;
        let terrain_image_hash = terrain_image.get_hash()?;
        //  Create an AssetUpload for the one texture.
        let image_asset_upload = AssetUpload::new(TileAssetType::BaseTexture(0), region, height_field, terrain_image_hash)?;    
        //  For sculpts, there's only one texture, the base texture, and only one face. Meshes are more complicated.
        let terrain_image_uuid_opt = image_asset_upload.get_asset_uuid(&mut self.conn)?;
        if let Some(uuid) = terrain_image_uuid_opt {
            log::info!("Terrain image asset already exists, reusing: {} UUID: {:?}", &image_asset_upload.asset_name, uuid);
            self.stats.assets_reused += 1;
        } else {
            let mut terrain_image_path = self.folder_generator.next_path()?;
            terrain_image_path.push(image_asset_upload.asset_name.to_owned() + ".png");
            let terrain_image_img = terrain_image.image.unwrap();
            terrain_image_img.save(&terrain_image_path)?;
            log::info!("Terrain image file saved: \"{}\"", terrain_image_path.display());
            println!("Terrain file: \"{}\"", terrain_image_path.display());
            //  Use it to create a new tile_asset entry with no UUID.
            //  Sculpts have only one face.
            let last_modified = terrain_image.last_modified;
            let new_tile_created = image_asset_upload.insert_tile_without_uuid(&mut self.conn, last_modified)?;
            if !new_tile_created {
                //  This ought not to happen much, if at all. If it happens generating and uploading were probably out of sequence.
                log::warn!("Duplicate tile image: {:?}", image_asset_upload);
            }
            self.stats.assets_generated += 1;      
        }
        //  Now we can generate the initial impostor database row.
        //  Sculpts have one face. They have no emissive texture. That's for meshes, in future.
        let face_0 = RegionImpostorFaceData {
            base_texture_uuid: terrain_image_uuid_opt,
            emissive_texture_uuid: None,
            base_texture_hash: hash_to_hex(terrain_image_hash),
            emissive_texture_hash: None
        };      
        let impostor_data =  InitialImpostors::assemble_region_impostor_data(TileType::Sculpt, region, height_field, viz_group_id, &hash_to_hex(sculpt_hash),
            sculpt_uuid_opt, &[face_0]);
        log::debug!("Region impostor data: {:?}", impostor_data);
        InitialImpostors::add_impostor(&mut self.conn, impostor_data)?;
        Ok(())
    }

    /// Build the impostor as a glTF mesh.
    pub fn build_impostor_mesh(
        &mut self,
        _region: &RegionData,
        _height_field: &HeightField,
        _viz_group_id: u32,
    ) -> Result<(), Error> {
        todo!("glTF mesh generation is not implemented yet");
    }
    
    /// Build an impostor for LOD N.
    fn build_impostor_for_lod(&mut self, region: &RegionData, _region_region_size_opt: Option<(u32, u32)>, viz_group_id: u32) -> Result<(), Error> {
        log::info!("Region \"{}\", LOD {} starting.", region.name, region.lod);
        let height_field = if region.lod == 0 {
            self.get_height_field_one_region(
                region.grid.clone(),
                region.region_loc_x,
                region.region_loc_y,
            )?
        } else {
            self.get_height_field_multi_region(
                region.grid.clone(),
                region.region_loc_x,
                region.region_loc_y,               
                (region.region_size_x, region.region_size_y),
                region.lod,
            )?
        };
        self.build_impostor(
            region,
            &height_field,
            viz_group_id,
        )?;
        log::info!("Region \"{}\", LOD {} built.", region.name, region.lod);
        Ok(())
    }
    
    /// Process group, multi-LOD version
    fn process_group(&mut self, group: Vec<RegionData>, viz_group_id: u32) -> Result<(), Error> {
        log::info!("Group #{}: {} entries.", viz_group_id, group.len());
        let region_size_opt = homogeneous_group_size(&group);
        if region_size_opt.is_some() && group.len() > 1 {
            //  Do the LOD thing.
            for region in TileLods::new(group) {
                self.build_impostor_for_lod(&region, region_size_opt, viz_group_id)?;
            }
        } else {
            //  LOD 0 only.
            for region in group {
                self.build_impostor_for_lod(&region, None, viz_group_id)?;
            }
        }
        Ok(())
    }

    /// Process one grid, with multiple visibilty groups
    pub fn process_grid(&mut self, mut completed_groups: CompletedGroups) -> Result<(), Error> {
        //  Sort by length, biggest groups first.
        completed_groups.sort_by(|a, b| b.len().partial_cmp(&a.len()).unwrap());
        for (viz_group_id, group) in completed_groups.into_iter().enumerate() {
            self.process_group(group, viz_group_id.try_into().unwrap())?;
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
    //  Clear old impostors from initial impostors.
    InitialImpostors::clear_grid(&mut terrain_generator.conn, &grid)?;
    let grid_entry = grids.pop().unwrap(); // get the one grid
    terrain_generator.process_grid(grid_entry)?;
    println!("Statistics:\n{}", terrain_generator.stats);
    log::info!("Statistics:\n{}", terrain_generator.stats);
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

