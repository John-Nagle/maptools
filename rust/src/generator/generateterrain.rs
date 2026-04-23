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
mod fetchtextures;
mod fetchbonniebots;
use anyhow::{anyhow, Error};
use common::{RegionData, HeightField, RegionImpostorFaceData, InitialImpostors};
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
use common::{hash_to_hex, AssetUpload, TileAssetType, RectU32};
use fetchbonniebots::{BonnieBotsBasicRegion, SL_GRID, SL_REGION_SIZE, TERRAIN_DATA_DIM, fetch_height_field};
use fetchtextures::{FetchTextures};
use ureq::{Agent};
use chrono::Utc;
use std::time::Duration;

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
/// Files per directory. A convenient size. Per prim limit is supposedly 10,000, but we hit some viewer limit for cut and paste.
const FILES_PER_DIRECTORY: usize = 200;
/// Force regen of all sculpts. Test use only.
const FORCE_SCULPT_REGEN: bool = false;

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
    
    /// Clear cache at start of each viz group.
    fn clear(&mut self) {
        self.cache.clear();
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
    agent: Agent,
    /// The run options
    run_opts: RunOpts,
    /// Output directory
    folder_generator_opt: Option<FolderGenerator>,
    /// The height field cache
    height_field_cache: HeightFieldCache,
    /// Statistics
    stats: TerrainGeneratorStats,
}

impl TerrainGenerator {
    /// Usual new.
    pub fn new(
        conn: PooledConn,
        run_opts: RunOpts,
    ) -> Self {
        const TIMEOUT_CONNECT: Duration = Duration::from_secs(15);
        const TIMEOUT_GLOBAL: Duration = Duration::from_secs(120);
        //  HTTP connection pool, used to validate UUIDs against asset server.
        let config = Agent::config_builder()
            .timeout_connect(Some(TIMEOUT_CONNECT))
            .timeout_global(Some(TIMEOUT_GLOBAL))
            .user_agent(TERRAIN_GENERATOR_USER_AGENT)
            .build();
        let agent: Agent = config.into();
        let folder_generator_opt = if let Some(outpath) = &run_opts.outpath_opt {
            Some(FolderGenerator::new(outpath, FILES_PER_DIRECTORY))
        } else {
            None
        };
        Self {
            conn,
            agent,
            run_opts,
            folder_generator_opt,
            height_field_cache: HeightFieldCache::new(),
            stats: TerrainGeneratorStats::new(),
        }
    }

    /// Build visibility group info from database
    pub fn transitive_closure_orig(&mut self, grid: &str) -> Result<Vec<CompletedGroups>, Error> {
        let mut vizgroups = VizGroups::new(self.run_opts.corners_touch_connects);
        let mut grids = Vec::new();
        log::info!("Build start"); // ***TEMP***
                                   //  The loop here is sequential data processing with control breaks when an index field changes.
        const SQL_SELECT: &str = r"SELECT grid, region_loc_x, region_loc_y, region_size_x, region_size_y, name FROM raw_terrain_heights WHERE LOWER(grid) = :grid ORDER BY grid, region_loc_x, region_loc_y ";
        let run_opts = &self.run_opts;
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
                //  Clip to regions of interest. Test feature.
                let keep = run_opts.keep_region_of_interest(&region_data);
                if keep {
                    if let Some(completed_groups) = vizgroups.add_region_data(region_data) {
                       grids.push(completed_groups);
                    }
                }
            },
        )?;
        grids.push(vizgroups.end_grid());
        Ok(grids)
    }
    
    /// Build list of region groups using BonnieBots input. One grid.
    pub fn transitive_closure_bb(&mut self, grid: &str) -> Result<CompletedGroups, Error> {
        if grid != SL_GRID {
            return Err(anyhow!("BonnieBots mode only works for grid {}", SL_GRID));
        }
        let regions = BonnieBotsBasicRegion::fetch_region_list_json(&mut self.agent)?;
        let initial_region_list =
            BonnieBotsBasicRegion::from_json(&regions)?;
        //  Filter regions based on clip filter.
        let mut region_list: Vec<_> = initial_region_list.iter().filter(|r| self.run_opts.keep_region_of_interest(r)).collect();
        log::info!("BonnieBots: {} regions before fiter, {} after filter.", initial_region_list.len(), region_list.len());
        //  Get the visgroups data.
        log::info!("Vizgroups build start"); 
        //  Sort by region_data by x, y, grid
        region_list.sort_by(|a, b| (&a.grid, a.region_loc_x, a.region_loc_y).cmp(&(&b.grid, b.region_loc_x, b.region_loc_y)));
        let mut viz_groups = VizGroups::new(false);
        
        for item in region_list {
            let grid_break = viz_groups.add_region_data(item.clone());
            //  BonnieBots only does one grid, so there's no control break.
            assert_eq!(grid_break, None);
        }
        let mut results = viz_groups.end_grid();
        results.sort_by(|a, b| b.len().partial_cmp(&a.len()).unwrap());
        log::info!("Vizgroups build end");
        Ok(results)
    }
    
    /// Dump completed groups to log. One grid.
    pub fn dump_completed_groups(&self, results: &CompletedGroups) {
        //  Display results
        log::info!("Viz groups: {}", results.len());
        //  Debug print
        for viz_group in results.iter() {
            if viz_group.len() <= 1 {
                continue
            }
            log::info!("Reachable group, {} regions, first region: {:?}", viz_group.len(), viz_group[0].name);
            for n in 0..viz_group.len().min(30) {
                log::debug!("  {}", viz_group[n]);
            }
        }
    }
    
    /// Switch data source depending on BonnieBots mode.
    pub fn get_height_field_one_region(&mut self,
        grid: String,
        name: &str,
        region_loc_x: u32,
        region_loc_y: u32,
    ) -> Result<HeightField, Error> {
        if self.run_opts.bonnie_bots_mode {
            self.get_height_field_one_region_bb(grid, name, region_loc_x, region_loc_y)
        } else {
            self.get_height_field_one_region_orig(grid, name, region_loc_x, region_loc_y)
        }         
    }
    
    /// Get elevation data for one region, BonnieBots mode.
    pub fn get_height_field_one_region_bb(&mut self,
        grid: String,
        name: &str,
        region_loc_x: u32,
        region_loc_y: u32,
    ) -> Result<HeightField, Error> {
        if &grid != SL_GRID {
            return Err(anyhow!("Grid requested is {}. Only allowed grid in Bonniebots mode is {}", grid, SL_GRID));
        }
        let height_field_opt = fetch_height_field(&mut self.agent, region_loc_x / SL_REGION_SIZE, region_loc_y / SL_REGION_SIZE)?;
        let height_field = if let Some(height_field) = height_field_opt {
            height_field
        } else {
            //  TROUBLE - no height field available
            log::error!("No Bonniebots height field for \"{}\" at ({},{})", name, region_loc_x, region_loc_y);
            Self::create_fake_height_field_bb()
        };
        //  Cache for later generation of lower LODs
        let key = RegionLodKey { lod: 0, region_loc_x, region_loc_y };
        self.height_field_cache.insert(key, height_field.clone());
        Ok(height_field)
    }
    
    /// Create fake height field for missing data.
    /// It's flat.
    /// This is just until BonnieBots gets better coverage.
    fn create_fake_height_field_bb() -> HeightField {
        const FAKE_HEIGHT_FIELD_WATER_HEIGHT: f32 = 20.0;   /// Fake water height.
        const FAKE_HEIGHT_FIELD_HEIGHT: f32 = FAKE_HEIGHT_FIELD_WATER_HEIGHT + 1.0;   // Fake water height.
        let heights = array2d::Array2D::filled_with(FAKE_HEIGHT_FIELD_HEIGHT, TERRAIN_DATA_DIM, TERRAIN_DATA_DIM);
        HeightField::new(heights, SL_REGION_SIZE, SL_REGION_SIZE, FAKE_HEIGHT_FIELD_WATER_HEIGHT)      
    }

    /// Get elevation data for one region.
    /// Original mode, from our own database.
    pub fn get_height_field_one_region_orig(
        &mut self,
        grid: String,
        _name: &str,
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
        fetcher: &mut FetchTextures,
        region: &RegionData,
        height_field: &HeightField,
        viz_group_id: u32,
    ) -> Result<(), Error> {
        if self.folder_generator_opt.is_none() {
            //  Test mode, not outputting anything
            return Ok(())
         }
        if self.run_opts.generate_mesh {
            self.build_impostor_mesh(
                fetcher,
                region,
                height_field,
                viz_group_id,
            )
        } else {
            self.build_impostor_sculpt(
                fetcher,
                region,
                height_field,
                viz_group_id,
            )
        }
    }

    /// Build the impostor as a sculpt.
    pub fn build_impostor_sculpt(
        &mut self,
        fetcher: &mut FetchTextures,
        region: &RegionData,
        height_field: &HeightField,
        viz_group_id: u32,
    ) -> Result<(), Error> {
        log::info!("Generating sculpt for \"{}\": {}", region.name, height_field);
        let tile_key = ((region.region_loc_x, region.region_loc_y), region.lod);
        if !fetcher.tile_has_land(tile_key) {
            log::debug!("All water, tile not generated, at {:?}", tile_key);
            return Ok(());
        }
        // TerrainSculpt was translated from Python with an LLM. NEEDS WORK
        //  Do sculpt
        let mut terrain_sculpt = TerrainSculpt::new(height_field.water_height);
        let (scale, offset, elevs) = height_field.into_sculpt_array()?;
        terrain_sculpt.setelevs(elevs, scale as f64, offset as f64);
        terrain_sculpt.makeimage();
        let sculpt_hash = terrain_sculpt.get_hash()?;
        //  Create an AssetUpload for the one texture.
        let sculpt_asset_upload = AssetUpload::new(TileAssetType::SculptTexture, region, &terrain_sculpt, sculpt_hash)?;    
        let sculpt_uuid_opt = sculpt_asset_upload.get_asset_uuid(&mut self.conn)?;
        if let Some (uuid) = sculpt_uuid_opt && !FORCE_SCULPT_REGEN {
            log::info!("Sculpt image asset already exists: {} UUID: {:?}", sculpt_asset_upload.asset_name, uuid);
            self.stats.assets_reused += 1;
        } else {
            let sculpt_image = terrain_sculpt.image.as_ref().unwrap();
            let mut sculpt_image_path = self.folder_generator_opt.as_mut().unwrap().next_path()?;
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
        let mut terrain_image = TerrainSculptTexture::new(&region);
        terrain_image.makeimage(fetcher, TERRAIN_SCULPT_TEXTURE_SIZE)?;
        let terrain_image_hash = terrain_image.get_hash()?;
        //  Create an AssetUpload for the one texture.
        let image_asset_upload = AssetUpload::new(TileAssetType::BaseTexture(0), region, &terrain_sculpt, terrain_image_hash)?;    
        //  For sculpts, there's only one texture, the base texture, and only one face. Meshes are more complicated.
        let terrain_image_uuid_opt = image_asset_upload.get_asset_uuid(&mut self.conn)?;
        if let Some(uuid) = terrain_image_uuid_opt {
            log::info!("Terrain image asset already exists, reusing: {} UUID: {:?}", &image_asset_upload.asset_name, uuid);
            self.stats.assets_reused += 1;
        } else {
            let mut terrain_image_path = self.folder_generator_opt.as_mut().unwrap().next_path()?;
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
        let impostor_data =  InitialImpostors::assemble_region_impostor_data(&terrain_sculpt, region, viz_group_id, &hash_to_hex(sculpt_hash),
            sculpt_uuid_opt, &[face_0]);
        log::debug!("Region impostor data: {:?}", impostor_data);
        InitialImpostors::add_impostor(&mut self.conn, impostor_data)?;
        Ok(())
    }

    /// Build the impostor as a glTF mesh.
    pub fn build_impostor_mesh(
        &mut self,
        _fetcher: &mut FetchTextures,
        _region: &RegionData,
        _height_field: &HeightField,
        _viz_group_id: u32,
    ) -> Result<(), Error> {
        todo!("glTF mesh generation is not implemented yet");
    }
    
    /// Build an impostor for LOD N.
    fn build_impostor_for_lod(&mut self, fetcher: &mut FetchTextures, region: &RegionData, viz_group_id: u32) -> Result<(), Error> {
        log::info!("Region \"{}\", LOD {} starting.", region.name, region.lod);
        let height_field = if region.lod == 0 {
            self.get_height_field_one_region(
                region.grid.clone(),
                &region.name,            
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
            fetcher,
            region,
            &height_field,
            viz_group_id,
        )?;
        log::info!("Region \"{}\", LOD {} built.", region.name, region.lod);
        Ok(())
    }
    
    /// Process group, multi-LOD version
    fn process_group(&mut self, group: Vec<RegionData>, viz_group_id: u32) -> Result<(), Error> {
        self.height_field_cache.clear();
        log::info!("Visibility group #{}: {} entries.", viz_group_id, group.len());
        let region_size_opt = homogeneous_group_size(&group);
        if region_size_opt.is_some() && group.len() > 1 {
            //  Do the LOD thing.
            let mut fetcher = FetchTextures::new(&self.agent, &group, region_size_opt);
            for region in TileLods::new(group) {
                self.build_impostor_for_lod(&mut fetcher, &region, viz_group_id)?;
            }
        } else {
            //  LOD 0 only.
            let mut fetcher = FetchTextures::new(&self.agent, &group, region_size_opt);
            for region in group {
                self.build_impostor_for_lod(&mut fetcher, &region, viz_group_id)?;
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

/// Put the command line parameters into one structure
#[derive(Debug, Clone)]
pub struct RunOpts {
    /// Output directory
    pub outpath_opt: Option<PathBuf>,
    /// Grid
    pub grid: String,
    /// URL prefix opt - for checking UUIDs
    pub url_prefix_opt: Option<String>,
    /// Generate mesh? Default is sculpt.
    pub generate_mesh: bool,
    /// BonnieBots mode - use BonnieBots data, not our own database
    pub bonnie_bots_mode: bool,
    /// Clip rectangles -- only accept regions in these rectangles, if non-null.
    pub clip_rectangles: Vec<RectU32>,
    /// Corners_touch -- true for Open Simulator grids, where touching corners means reachable
    pub corners_touch_connects: bool,
    /// Verbose mode
    pub verbose: bool,
}

impl RunOpts {
    /// New, from options on command line
    pub fn new_from_options(matches: &getopts::Matches) -> Result<Self, Error> {
        let verbose = matches.opt_present("v");
        let grid_opt = matches.opt_str("g");
        let url_prefix_opt = matches.opt_str("p");
        let generate_mesh = matches.opt_present("m");
        let bonnie_bots_mode = matches.opt_present("b");
        let corners_touch_connects = false;  // ***MORE***
        let grid = if let Some(grid) = grid_opt {
            grid.trim().to_lowercase()
        } else {
            return Err(anyhow!("No grid name given."));
        };
        let outpath_opt = if let Some(s) = matches.opt_str("o") { 
            Some(PathBuf::from(&s))
        } else {
            None
        };
        const SL_REGION_SIZE: u32 = 256;  // ***MOVE**
        //  Clipping to part of the grid. 
        //  Mostly for test purposes.
        //  In units of meters
        let mut clip_rectangles: Vec<RectU32> = matches
            .opt_strs("clipm")
            .iter()
            .map(|c: &String| RectU32::parse(&c)).collect::<Result<Vec<RectU32>, Error>>()?;
        //  In units of regions
        let clips_regions: Vec<RectU32> = matches
            .opt_strs("clip")
            .iter()
            .map(|c: &String| RectU32::parse(&c)).collect::<Result<Vec<RectU32>, Error>>()?;
        //  Scale to meters
        let clips_regions_m: Vec<RectU32> = 
            clips_regions
            .iter()
            .map(|c: &RectU32 | RectU32::new([c.ll[0]*SL_REGION_SIZE, c.ll[1]*SL_REGION_SIZE], [c.ur[0]*SL_REGION_SIZE, c.ur[1]*SL_REGION_SIZE]))
            .collect();
            //  Combine
            clip_rectangles.extend(clips_regions_m);    
        
        Ok(Self {
            outpath_opt,
            grid,
            url_prefix_opt,
            generate_mesh,
            bonnie_bots_mode,
            clip_rectangles,
            corners_touch_connects,
            verbose,
        })
    }
    
    /// Do we want to keep this region? 
    /// Checks against command line clip list.
    /// Mostly for testing.
    pub fn keep_region_of_interest(&self, region_data: &RegionData) -> bool {
        if !self.clip_rectangles.is_empty() {
            let region_rect = RectU32::new(
                [region_data.region_loc_x, region_data.region_loc_y],
                [region_data.region_loc_x + region_data.region_size_x, region_data.region_loc_y + region_data.region_size_y]);
            //  Passes if in any clip rectangle.
            self.clip_rectangles.iter().find(|r: &&RectU32| r.overlaps(&region_rect)).is_some()
        } else {
            //  No clip list, everything passes
            true
        }
    }

}

/// Actually do the work
fn run(pool: Pool, run_opts: RunOpts) -> Result<(), Error> {
    let conn = pool.get_conn()?;
    let mut terrain_generator =
        TerrainGenerator::new(conn, run_opts.clone());
    let grid_entry = if run_opts.bonnie_bots_mode {
        terrain_generator.transitive_closure_bb(&run_opts.grid)?
    } else {
        let mut grids = terrain_generator.transitive_closure_orig(&run_opts.grid)?;
        if grids.is_empty() {
            return Err(anyhow!("Grid \"{}\" not found.", &run_opts.grid));
        }

        if grids.len() != 1 {
            return Err(anyhow!(
                "More than one grid found but SQL should return only one grid."
            ));
        }
        grids.pop().unwrap() // get the one grid
    };
    //  Filter out vizgroups too tiny to impostor
    const MINIMUM_REGIONS_PER_VIZGROUP: usize = 2;
    let grid_entry: Vec<_> = grid_entry.into_iter().filter(|vg| vg.len() >= MINIMUM_REGIONS_PER_VIZGROUP).collect();
    //  Log group info
    terrain_generator.dump_completed_groups(&grid_entry);
    //  Clear old impostors from initial impostors.
    if run_opts.outpath_opt.is_some() {
        //  But only in production mode
        InitialImpostors::clear_grid(&mut terrain_generator.conn, &run_opts.grid)?;
    }
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
fn setup() -> Result<(Pool, RunOpts), Error> {
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
    opts.optmulti("k", "clip", "Clip rectangle in regions for area to impostor", "(n,n)-(n,n)");
    opts.optmulti("", "clipm", "Clip rectangle in meters for area to impostor", "(n,n)-(n,n)");
    opts.optflag("b", "bonniebots", "Use Bonniebots region data.");
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
    let credsfile = matches.opt_str("c");
    if credsfile.is_none() {
        print_usage(&program, opts);
        return Err(anyhow!("Required command line options missing"));
    }
    let credsfile = credsfile.unwrap();
    let run_opts = RunOpts::new_from_options(&matches)?;
    println!("Options: {:?}", run_opts);
    if let Some(outpath) = &run_opts.outpath_opt {
     // Create the output directory, empty.
        std::fs::create_dir_all(outpath)?;
    } else {
        println!("No output directory, this is a test run and will not write to the database.");
    };
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
    if run_opts.verbose {
        println!("Connected to database.");
    }
    log::info!("Connected to database.");
    //  Setup complete. Return what's needed to run.
    Ok((pool, run_opts))
}

/// Main program.
/// Setup, then run.
fn main() {
    logger();
    match setup() {
        Ok((pool, run_opts)) => match run(pool, run_opts) {
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

