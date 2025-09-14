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
use anyhow::{Error, anyhow};
use chrono::{NaiveDateTime, Utc};
use envie::Envie;
use getopts::Options;
use log::LevelFilter;
use mysql::prelude::{AsStatement, Queryable};
use mysql::{Conn, Opts, OptsBuilder, Pool};
use mysql::{PooledConn, params};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Write;
use common::Credentials;
use common::{UploadedRegionInfo, ElevsJson};

mod vizgroup;
use vizgroup::{RegionData, VizGroups, CompletedGroups};

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
/*
///  Our data as uploaded from SL/OS in JSON format
// "{\"region\":\"Vallone\",\"scale\":1.092822,\"offset\":33.500740,\"waterlev\":20.000000,\"regioncoords\":[1807,1199],
//  \"elevs\":[\"E7CAACA3A5A8ACAEB0B2B5B9BDC0C4C5C5C3C0BDB9B6B3B2B2B3B4B7BBBFC3C7CBCED1D3D5D5D4CFC4B5A4"";
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UploadedRegionInfo {
    /// Grid name
    grid: String,
    /// Position of region in world, meters.
    pub region_coords: [u32; 2],
    /// Region size. 256 x 256 if ommitted.
    size: Option<[u32; 2]>,
    /// Region name
    name: String,
    /// Height data, a long set of hex data.  
    elevs: Vec<String>,
    /// Scale factor for elevs
    scale: f32,
    /// Offset factor for elevs
    /// actual = input*scale + offset
    offset: f32,
    //  Water level
    pub water_lev: f32,
}

/// Elevations as JSON data
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ElevsJson {
    /// Offset and scale for elevation data
    offset: f32,
    /// Apply scale first, then offset.
    scale: f32,
    /// Height data, a long set of hex data.  
    elevs: Vec<String>,
}

impl ElevsJson {
    /// Get elevations as numbers before offsetting.
    /// Input is a hex string representing one elev per byte.
    pub fn get_unscaled_elevs(&self) -> Result<Vec<Vec<u8>>, Error> {
        let elevs: Result<Vec<_>, _> = self.elevs.iter().map(|s| hex::decode(s)).collect();
        Ok(elevs?)
    }
}

impl UploadedRegionInfo {
    /// Parse from string
    pub fn parse(s: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(s)?)
    }

    /// Get size, applying default region size for non-varregions
    pub fn get_size(&self) -> [u32; 2] {
        if let Some(size) = self.size {
            size
        } else {
            [DEFAULT_REGION_SIZE, DEFAULT_REGION_SIZE]
        }
    }

    /// Get grid in canonial lowercase format
    pub fn get_grid(&self) -> String {
        self.grid.to_lowercase()
    }

    /// Get region name in canonical lowercase format
    pub fn get_name(&self) -> String {
        self.name.to_lowercase()
    }

    /// Get elevs as a blob for SQL.
    /// Elevs are a vector of rows of hex strings at this point.
    pub fn get_elevs_as_blob(&self) -> Result<Vec<u8>, Error> {
        let elevs_blob: Vec<_> = self.get_unscaled_elevs()?.into_iter().flatten().collect();
        Ok(elevs_blob)
    }
    /// Get elevations as numbers before offsetting.
    /// Input is a hex string representing one elev per byte
    /// Output is an array of hex strings.
    pub fn get_unscaled_elevs(&self) -> Result<Vec<Vec<u8>>, Error> {
        let elevs: Result<Vec<_>, _> = self.elevs.iter().map(|s| hex::decode(s)).collect();
        Ok(elevs?)
    }

    /// Scale the elevations
    pub fn get_scaled_elevs(&self) -> Result<Vec<Vec<f32>>, Error> {
        todo!();
        //////Ok(self.get_unscaled_elevs()?.iter().map(|&v| ((v as f32) / 256.0) * self.scale + self.offset).collect())
    }
}
*/
/*
///  Our handler
struct TerrainUploadHandler {
    /// MySQL onnection pool. We only use one.
    pool: Pool,
    /// Active MySQL connection.
    conn: PooledConn,
}
impl TerrainUploadHandler {
    /// Usual new. Saves connection pool for use.
    pub fn new(pool: Pool) -> Result<Self, Error> {
        let conn = pool.get_conn()?;
        Ok(Self {
            pool,
            conn,
        })
    }

    /// SQL insert for new item
    fn do_sql_insert(&mut self, region_info: UploadedRegionInfo, params: &HashMap<String, String>) -> Result<(), Error> {
        const SQL_INSERT: &str = r"INSERT INTO raw_terrain_heights (grid, region_coords_x, region_coords_y, size_x, size_y, name, scale, offset, elevs,  water_level, creator)
            VALUES (:grid, :region_coords_x, :region_coords_y, :size_x, :size_y, :name, :scale, :offset, :elevs, :water_level, :creator)";
        //  ***NEED TO FIX THIS FOR Open Simulator***
        let creator = params.get(OWNER_NAME).ok_or_else(|| anyhow!("This request is not from Second Life/Open Simulator"))?.trim();
        let values = params! {
            //////"table" => RAW_TERRAIN_HEIGHTS,
            "grid" => region_info.grid.clone(),
            "region_coords_x" => region_info.region_coords[0],
            "region_coords_y" => region_info.region_coords[1],
            "size_x" => region_info.get_size()[0],
            "size_y" => region_info.get_size()[1],
            "name" => region_info.name.clone(),
            "scale" => region_info.scale,
            "offset" => region_info.offset,
            "elevs" => region_info.get_elevs_as_blob()?,
            "water_level" => region_info.water_lev,
            "creator" => creator };
        log::debug!("SQL insert: {:?}", values);
        self.conn.exec_drop(SQL_INSERT, values)?;
        log::debug!("SQL insert succeeded.");
        Ok(())
    }

    fn do_sql_update(&mut self, region_info: UploadedRegionInfo, params: &HashMap<String, String>) -> Result<(), Error> {
        const SQL_INSERT: &str = r"INSERT INTO raw_terrain_heights (grid, region_coords_x, region_coords_y, size_x, size_y, name, scale, offset, elevs,  water_level, creator)
            VALUES (:grid, :region_coords_x, :region_coords_y, :size_x, :size_y, :name, :scale, :offset, :elevs, :water_level, :creator)";
        //  ***NEED TO FIX THIS FOR Open Simulator***
        let creator = params.get(OWNER_NAME).ok_or_else(|| anyhow!("This request is not from Second Life/Open Simulator"))?.trim();
        let values = params! {
            //////"table" => RAW_TERRAIN_HEIGHTS,
            "grid" => region_info.grid.clone(),
            "region_coords_x" => region_info.region_coords[0],
            "region_coords_y" => region_info.region_coords[1],
            "size_x" => region_info.get_size()[0],
            "size_y" => region_info.get_size()[1],
            "name" => region_info.name.clone(),
            "scale" => region_info.scale,
            "offset" => region_info.offset,
            "elevs" => region_info.get_elevs_as_blob()?,
            "water_level" => region_info.water_lev,
            "creator" => creator };
        log::debug!("SQL insert: {:?}", values);
        self.conn.exec_drop(SQL_INSERT, values)?;
        log::debug!("SQL insert succeeded.");
        Ok(())
    }

    /// Parse a request
    fn parse_request(b: &[u8], _env: &HashMap<String, String>) -> Result<UploadedRegionInfo, Error> {
        //  Should be UTF-8. Check.
        let s = core::str::from_utf8(b)?;
        if s.trim().is_empty() {
            return Err(anyhow!("Empty request. JSON was expected"));
        }
        log::info!("Uploaded JSON:\n{}", s);
        //  Should be valid JSON
        Ok(UploadedRegionInfo::parse(s)?)
    }

    /// Handle request.
    ///
    /// Start a database transaction.
    /// Check if this data is the same as any stored data for this region.
    /// If yes, just update confirmation user and time.
    /// If no, replace old data entirely.
    fn process_request(&mut self, region_info: UploadedRegionInfo, params: &HashMap<String, String>) -> Result<String, Error> {
        let msg = format!("Region info:\n{:?}", region_info);
        //  Initial test of SQL
        self.do_sql_insert(region_info, params)?;   // ***TEMP***
        //////let msg = "Test OK".to_string(); // ***TEMP***
        Ok(msg)
    }
}
//  Our "handler"
impl Handler for TerrainUploadHandler {
    fn handler(
        &mut self,
        out: &mut dyn Write,
        request: &Request,
        env: &HashMap<String, String>,
    ) -> Result<(), Error> {
        //  We have a request. It's supposed to be in JSON.
        //  Parse. Error 400 with message if fail.
        match Self::parse_request(&request.standard_input, env) {
            Ok(req) => {
                log::info!("Request made: {:?} env {:?}", req, env);
                let params = request.params.as_ref().ok_or_else(|| anyhow!("No HTTP parameters found"))?;
                //  Process. Error 500 if fail.
                match self.process_request(req, &params) {
                    Ok(msg) => {
                        //  Success. Send a plain "OK"
                        let http_response = Response::http_response("text/plain", 200, "OK");
                        //  Return something useful.
                        let b = msg.into_bytes();
                        Response::write_response(out, request, http_response.as_slice(), &b)?;
                    }
                    Err(e) => {
                       let http_response = Response::http_response("text/plain", 500, format!("Problem processing request: {:?}", e).as_str());
                        Response::write_response(out, request, http_response.as_slice(), &[])?;
                    }
                }
            }
            Err(e) => {
                let http_response = Response::http_response("text/plain", 400, format!("Incorrect request: {:?}", e).as_str());
                //  Return something useful.
                //////let b = format!("Env: {:?}\nParams: {:?}\n", env, request.params).into_bytes();
                let b = [];
                Response::write_response(out, request, http_response.as_slice(), &b)?;
            }
        }
        Ok(())
    }
}

/// Run the responder.
pub fn run_responder() -> Result<(), Error> {
    log::info!("Environment: {:?}", std::env::vars());
    //  Set up in and out sockets.
    //  Communication with the parent process is via a UNIX socket.
    //  This is a pain to set up, because UNIX sockets are badly mis-matched
    //  to parent/child process communication.
    //  See init_fcgi for how it is done.
    let listener = init_fcgi()?;
    //  Accept a connection on the listener socket. This hooks up
    //  input and output to the parent process.
    let (socket, _addr) = listener.accept()?;
    let outsocket = socket.try_clone()?;
    let mut instream = std::io::BufReader::new(socket);
    let mut outio = std::io::BufWriter::new(outsocket);
    //  Connect to the database
    let mut terrain_upload_handler = TerrainUploadHandler::new(pool)?;
    //  Run the FCGI server.
    minifcgi::run(&mut instream, &mut outio, &mut terrain_upload_handler)
}
*/

/// The terrain object generator
struct TerrainGenerator {
    /// Are regions with only corners touching adjacent?
    /// Set to true for Open Simulator grids
    corners_touch_connects: bool,
}

impl TerrainGenerator {

    pub fn new(corners_touch_connects: bool) -> Self {
        Self {
            corners_touch_connects
        }
    }

    /// Build visibility group info from database
    pub fn transitive_closure(&self, conn: &mut PooledConn, grid: &str) -> Result<Vec<CompletedGroups>, Error> {
        let mut vizgroups = VizGroups::new(self.corners_touch_connects);
        let mut grids = Vec::new();
        log::info!("Build start"); // ***TEMP***
        //  The loop here is sequential data processing with control breaks when an index field changes.
        const SQL_SELECT: &str = 
            r"SELECT grid, region_coords_x, region_coords_y, size_x, size_y, name FROM raw_terrain_heights WHERE LOWER(grid) = :grid ORDER BY grid, region_coords_x, region_coords_y ";
        let _all_regions = conn.exec_map(
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
    pub fn needed_regions(&self, completed_groups: &Vec::<CompletedGroups>) -> Result<(), Error> {
        todo!();
    }
    
    /// Get elevation data for one region.
    pub fn get_elevs_one_region(&self, grid: String, region_coords_x: u32, region_coords_y: u32, conn: &mut PooledConn) -> Result<UploadedRegionInfo, Error> {
        const SQL_SELECT: &str = 
            r"SELECT grid, region_coords_x, region_coords_y, size_x, size_y, name, scale, offset, elevs,  water_level
                FROM raw_terrain_heights
                WHERE LOWER(grid) = :grid, region_coords_x = : region_coords_x, region_coords_y = :region_coords_y";
        let grid_for_msg = grid.clone();
        let regions = conn.exec_map(
            SQL_SELECT,
            params! { grid, region_coords_x, region_coords_y },
            |(grid, region_coords_x, region_coords_y, size_x, size_y, name, scale, offset, elevs,  water_level)| {
                let region_coords = [region_coords_x, region_coords_y];
                let size = Some([size_x, size_y]);
                let water_lev = water_level;
                let _raw_elevs: Vec<u8> = elevs;
                let elevs = vec![];// ***TEMP***
                UploadedRegionInfo {
                    grid, region_coords, size, name, scale, offset, elevs,  water_lev}
            })?;
        if regions.is_empty() {
            return Err(anyhow!("No raw terrain data for region at ({},{}) on \"{}\"", region_coords_x, region_coords_y, grid_for_msg));
        }
        if regions.len() > 1 {
            //  Duplicate data - warning 
            //  SQL indices should make this impossible.
            log::error!("More than one region data set for region at ({},{}) on \"{}\"", region_coords_x, region_coords_y, grid_for_msg);
        }
        let region: UploadedRegionInfo = regions[0].clone();
        Ok(region)
    }
    
    /// Build impostor, either sculpt or mesh form.
    /// This collects the elevation data needed to build the impostor geometry.//
    //  ***NEED TO HANDLE MULTIPLE REGION IMPOSTORS.
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
    
    /// Build impostor, sculpt form.
    pub fn build_impostor_sculpt(&self, region_data: &RegionData, conn: &mut PooledConn) {
        todo!();
    }
    
}

/// Actually do the work
fn run(pool: Pool, outdir: String, grid: String, verbose: bool) -> Result<(), Error> {
    //////println!("{:?} {:?} {}", credsfile, outdir, verbose);
    let corners_touch_connects = false; // for now, SL only.
    let terrain_generator = TerrainGenerator::new(corners_touch_connects);    
    let mut conn = pool.get_conn()?;
    let _results = terrain_generator.transitive_closure(&mut conn, &grid)?;
    //  ***MORE***
    Ok(())
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}

/// Set up options, credentials, and database connection.
fn setup() -> Result<(Pool, String, String, bool), Error> {
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
    opts.optopt("g", "grid", "Only output for this grid", "NAME");
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
    if outdir.is_none() || credsfile.is_none() || grid.is_none() {
        print_usage(&program, opts);
        return Err(anyhow!("Required command line options missing"));
    }
    let credsfile = credsfile.unwrap();
    let outdir = outdir.unwrap();
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
    Ok((pool, outdir, grid, verbose))
}

/// Main program.
/// Setup, then run.
fn main() {
    logger();
    match setup() {
        Ok((pool, outdir, grid, verbose)) => match run(pool, outdir, grid, verbose) {
            Ok(_) => {
                if verbose {
                    println!("Done.");
                }
            }
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
