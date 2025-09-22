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
use anyhow::{anyhow, Error};
use chrono::{NaiveDateTime, Utc};
use common::Credentials;
use common::{HeightField, UploadedRegionInfo};
use envie::Envie;
use getopts::Options;
use log::LevelFilter;
use mysql::prelude::{AsStatement, Queryable};
use mysql::{params, PooledConn};
use mysql::{Conn, Opts, OptsBuilder, Pool};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

mod vizgroup;
use vizgroup::{CompletedGroups, RegionData, VizGroups};
mod sculptmaker;
use image::GrayImage;
use sculptmaker::TerrainSculpt;

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
const UPLOAD_CREDS_FILE: &str = "generate_credentials.txt";
/// Default region size, used on grids that don't do varregions.
const DEFAULT_REGION_SIZE: u32 = 256;
/// Table name
const RAW_TERRAIN_HEIGHTS: &str = "raw_terrain_heights";
/// Environment variables for obtaining owner info.
/// ***ADD VALUES FOR OPEN SIMULATOR***
const OWNER_NAME: &str = "HTTP_X_SECONDLIFE_OWNER_NAME";

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

/// The terrain object generator
struct TerrainGenerator {
    /// SQL connection
    conn: PooledConn,
    /// Output directory
    outdir: PathBuf,
    /// Are regions with only corners touching adjacent?
    /// Set to true for Open Simulator grids
    corners_touch_connects: bool,
    /// Generate glTF mesh if on.
    generate_mesh: bool,
}

impl TerrainGenerator {
    /// Usual new.
    pub fn new(
        conn: PooledConn,
        outdir: PathBuf,
        corners_touch_connects: bool,
        generate_mesh: bool,
    ) -> Self {
        Self {
            conn,
            outdir,
            corners_touch_connects,
            generate_mesh,
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
                };
                if let Some(completed_groups) = vizgroups.add_region_data(region_data) {
                    grids.push(completed_groups);
                }
            },
        )?;
        grids.push(vizgroups.end_grid());
        Ok(grids)
    }

    /// Which region impostors do we need to create?
    /// This filters the results of the transitive closure based on what's in the database and the servers.
    ///
    /// Transitive closure tells us if regions are in the same VizGroup.
    /// Then we must check the database of impostored regions to tie VizGroups to viz_group IDs.
    //  ***WE MAY HAVE TO MERGE AND SPLIT HERE***
    //  ***NEED TO RUN ALL EXISTING REGIONS THROUGH TRANSITIVE CLOSURE***
    pub fn needed_regions(&self, completed_groups: &Vec<CompletedGroups>) -> Result<(), Error> {
        todo!();
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
                    &elevs, samples_x, samples_y, size_x, size_y, scale, offset,
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
        Ok(height_field)
    }
    /*
        /// Build impostor, either sculpt or mesh form.
        /// This collects the elevation data needed to build the impostor geometry.//
        //  ***NEED TO HANDLE MULTIPLE-REGION IMPOSTORS.
        pub fn build_impostor(&self, region_data: &RegionData, conn: &mut PooledConn, use_mesh: bool) -> Result<(), Error> {
            log::info!("Building impostor for {}", region_data.name); //
            //  The loop here is sequential data processing with control breaks when an index field changes.
            let grid = region_data.grid.clone();
            let region_coords_x = region_data.region_coords_x;
            let region_coords_y = region_data.region_coords_y;
            const SQL_SELECT: &str =
                r"SELECT grid, region_coords_x, region_coords_y, size_x, size_y, name FROM raw_terrain_heights
                    WHERE LOWER(grid) = :grid, region_coords_x = : region_coords_x, region_coords_y = :region_data.region_coords_y";
            let _all_regions = conn.exec_map(
                SQL_SELECT,
                params! { region_coords_x, region_coords_y, grid },
                |(grid, region_coords_x, region_coords_y, size_x, size_y, name)| {
                    let region_data = RegionData {
                        grid,
                        region_coords_x,
                        region_coords_y,
                        size_x,
                        size_y,
                        name,
                    };
                },
            )?;
            todo!();
        }
    */

    /// Generate name for impostor asset file.
    /// Format: R-x-y-lod-name
    fn impostor_name(
        region_coords_x: u32,
        region_coords_y: u32,
        lod: u8,
        impostor_name: &str,
    ) -> String {
        let x = region_coords_x;
        let y = region_coords_y;
        format!("R-{}-{}-{}-{}", x, y, lod, impostor_name)
    }

    /// Build the impostor
    pub fn build_impostor(
        &mut self,
        region_coords_x: u32,
        region_coords_y: u32,
        lod: u8,
        impostor_name: &str,
        height_field: &HeightField,
    ) -> Result<(), Error> {
        if self.generate_mesh {
            self.build_impostor_mesh(
                region_coords_x,
                region_coords_y,
                lod,
                impostor_name,
                height_field,
            )
        } else {
            self.build_impostor_sculpt(
                region_coords_x,
                region_coords_y,
                lod,
                impostor_name,
                height_field,
            )
        }
    }

    /// Build the impostor as a sculpt.
    pub fn build_impostor_sculpt(
        &mut self,
        region_coords_x: u32,
        region_coords_y: u32,
        lod: u8,
        impostor_name: &str,
        height_field: &HeightField,
    ) -> Result<(), Error> {
        log::info!("Generating sculpt for \"{}\": {}", impostor_name, height_field);
        // TerrainSculpt was translated from Python with an LLM. NEEDS WORK
        let mut terrain_sculpt = TerrainSculpt::new(impostor_name);
        let (scale, offset, elevs) = height_field.into_sculpt_array()?;
        terrain_sculpt.setelevs(elevs, scale as f64, offset as f64);
        terrain_sculpt.makeimage();
        let img = terrain_sculpt.image.unwrap();
        let mut imgpath = self.outdir.clone();
        imgpath.push(impostor_name.to_owned() + ".png");
        log::info!("Sculpt image saved: \"{}\"", imgpath.display());
        img.save(imgpath)?;
        Ok(())
    }

    /// Build the impostor as a glTF mesh.
    pub fn build_impostor_mesh(
        &mut self,
        region_coords_x: u32,
        region_coords_y: u32,
        lod: u8,
        impostor_name: &str,
        height_field: &HeightField,
    ) -> Result<(), Error> {
        todo!("glTF mesh generation is not implemented yet");
    }

    /// Process one visibiilty group.
    /// There's a lot to do here.
    /// Temp version - just generates impostors for all single regions.
    pub fn process_group(&mut self, group: &Vec<RegionData>) -> Result<(), Error> {
        println!("Group: {} entries.", group.len()); // ***TEMP***
                                                     //  Dumb version, just do single-size regions.
        let lod = 0; // single regions only
        for region in group {
            let height_field = self.get_height_field_one_region(
                region.grid.clone(),
                region.region_coords_x,
                region.region_coords_y,
            )?;
            let impostor_name = Self::impostor_name(
                region.region_coords_x,
                region.region_coords_y,
                lod,
                &region.name,
            );
            self.build_impostor(
                region.region_coords_x,
                region.region_coords_y,
                lod,
                &impostor_name,
                &height_field,
            )?;
            println!("Region \"{}\": {}", region.name, height_field);
        }
        Ok(())
    }

    /// Process one grid, with multiple visibilty groups
    pub fn process_grid(&mut self, mut completed_groups: CompletedGroups) -> Result<(), Error> {
        completed_groups.sort_by(|a, b| b.len().partial_cmp(&a.len()).unwrap());
        for group in &completed_groups {
            self.process_group(group)?;
        }
        Ok(())
    }
}

/// Actually do the work
fn run(pool: Pool, outdir: PathBuf, grid: String, generate_mesh: bool) -> Result<(), Error> {
    //////println!("{:?} {:?} {}", credsfile, outdir, verbose);
    let corners_touch_connects = false; // for now, SL only.
    let conn = pool.get_conn()?;
    let mut terrain_generator =
        TerrainGenerator::new(conn, outdir, generate_mesh, corners_touch_connects);
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
fn setup() -> Result<(Pool, PathBuf, String, bool), Error> {
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
    opts.optflag("m", "mesh", "Generate glTF mesh, not sculpt image");
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
    let generate_mesh = matches.opt_present("m");
    if outdir.is_none() || credsfile.is_none() || grid.is_none() {
        print_usage(&program, opts);
        return Err(anyhow!("Required command line options missing"));
    }
    let credsfile = credsfile.unwrap();
    let outdir = PathBuf::from(&outdir.unwrap());
    let grid = grid.unwrap().trim().to_lowercase();
    // Create the output directory, empty.
    //  ***MORE***
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
    //////log::info!("Opts: {:?}", opts);
    let pool = Pool::new(opts)?;
    if verbose {
        println!("Connected to database.");
    }
    log::info!("Connected to database.");
    //  Setup complete. Return what's needed to run.
    Ok((pool, outdir, grid, generate_mesh))
}

/// Main program.
/// Setup, then run.
fn main() {
    logger();
    match setup() {
        Ok((pool, outdir, grid, mesh)) => match run(pool, outdir, grid, mesh) {
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
/*
#[test]
fn generate_terrain() {
    let creds = Credentials::new(UPLOAD_CREDS_FILE)?;
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
    //////log::info!("Opts: {:?}", opts);
    let pool = Pool::new(opts)?;
    log::info!("Connected to database.");
}
*/
