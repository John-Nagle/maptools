//! Upload Second Life / Open Simulator terrain to server
//! Part of the Animats impostor system
//!
//! A Second Life/Open Simulator LSL script records terrain heights when visiting
//! regions. It calls this FCGI responder to upload that data to a server.
//! Later processing turns that into objects viewable in world via the
//! region impostor system.
//!
//!     License: LGPL.
//!     Animats
//!     August, 2025.
//
#![forbid(unsafe_code)]
use anyhow::{Error, anyhow};
use chrono::{NaiveDateTime, Utc};
use log::LevelFilter;
use common::Credentials;
use common::init_fcgi;
use common::{Handler, Request, Response};
use common::{UploadedRegionInfo, ElevsJson};
use common::u8_to_elev;
use mysql::prelude::{AsStatement, Queryable};
use mysql::{Conn, Opts, OptsBuilder, Pool};
use mysql::{PooledConn, params};
use std::collections::HashMap;
use std::io::Write;
use sha2::{Sha256, Digest};

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
const UPLOAD_CREDS_FILE: &str = "upload_credentials.txt";
/// Table name
const RAW_TERRAIN_HEIGHTS: &str = "raw_terrain_heights";
/// Environment variables for obtaining owner info.
/// ***ADD VALUES FOR OPEN SIMULATOR***
const OWNER_NAME: &str = "HTTP_X_SECONDLIFE_OWNER_NAME";

/// Debug logging
fn logger() {
    //  Log file is openly visible as a web page.
    //  Only for debug tests.
    const LOG_FILE_NAME: &str = "logs/updatelog.txt";
    let _ = simplelog::CombinedLogger::init(vec![simplelog::WriteLogger::new(
        LevelFilter::Debug,
        simplelog::Config::default(),
        std::fs::File::create(LOG_FILE_NAME).expect("Unable to create log file"),
    )]);
    log::warn!("Logging to {:?}", LOG_FILE_NAME); // where the log is going
}

/// Change status for region data
#[derive(Debug)]
enum ChangeStatus {
    None, 
    NoChange,
    Changed 
}

///  Our handler
struct TerrainUploadHandler {
    /// MySQL onnection pool. We only use one.
    pool: Pool,
    /// Active MySQL connection.
    conn: PooledConn,
}
impl TerrainUploadHandler {
    /// Elevation error tolerance. Elevations are equal if within this tolerance.
    /// LSL llGround is slightly noisy.
    const ELEV_ERROR_TOLERANCE: f32 = 0.5;

    /// Usual new. Saves connection pool for use.
    pub fn new(pool: Pool) -> Result<Self, Error> {
        let conn = pool.get_conn()?;
        Ok(Self { pool, conn })
    }

    /// SQL insert for new item
    fn do_sql_insert(
        &mut self,
        region_info: &UploadedRegionInfo,
        params: &HashMap<String, String>,
    ) -> Result<(), Error> {
        const SQL_INSERT: &str = r"INSERT INTO raw_terrain_heights (grid, region_coords_x, region_coords_y, samples_x, samples_y, size_x, size_y, name, scale, offset, elevs,  water_level, creator) 
            VALUES
            (:grid, :region_coords_x, :region_coords_y, :samples_x, :samples_y, :size_x, :size_y, :name, :scale, :offset, :elevs, :water_level, :creator)";
        //  ***NEED TO FIX THIS FOR Open Simulator***
        let creator = params
            .get(OWNER_NAME)
            .ok_or_else(|| anyhow!("This request is not from Second Life/Open Simulator"))?
            .trim();
        let samples = region_info.get_samples()?;
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
        "samples_x" => samples[0],
        "samples_y" => samples[1],
        "water_level" => region_info.water_lev,
        "creator" => creator };
        log::debug!("SQL insert: {:?}", values);
        self.conn.exec_drop(SQL_INSERT, values)?;
        log::debug!("SQL insert succeeded.");
        Ok(())
    }
    
    /// Compare elevations within tolerance.
    /// LSL llGround is not totally repeatable.  We have to allow some error.
    fn check_elev_err_within_tolerance(elevs0: &[u8], elevs1: &[u8], scale: f32, offset: f32, tolerance: f32) -> bool {
        let elev_err = |a: u8, b: u8| (u8_to_elev(a, scale, offset) - u8_to_elev(b, scale, offset)).abs();
        let max_err_item_opt = elevs0.iter().zip(elevs1).max_by(|(a0, b0), (a1, b1)| {
            let aerr = elev_err(**a0, **b0);
            let berr = elev_err(**a1, **b1);
            aerr.total_cmp(&berr)
        });
        if let Some(max_err_item) = max_err_item_opt {
            let max_err = elev_err(*max_err_item.0, *max_err_item.1);
            if max_err > tolerance {
                log::warn!("Elevations differ by {:5}", max_err);
            }
            max_err < tolerance
        } else {
            // Not equal
            false 
        }
    }
    
    /// Is this a duplicate?
    fn do_sql_unchanged_check(
        &mut self,
        region_info: &UploadedRegionInfo,
        params: &HashMap<String, String>,
    ) -> Result<ChangeStatus, Error> {
        
        let samples = region_info.get_samples()?;
        let grid = &region_info.grid;
        let region_coords_x = region_info.region_coords[0];
        let region_coords_y = region_info.region_coords[1];
        let new_elevs= region_info.get_elevs_as_blob()?;
        const SQL_SELECT: &str = r"SELECT size_x, size_y, samples_x, samples_y, scale, offset, elevs, name, water_level
            FROM raw_terrain_heights
            WHERE LOWER(grid) = :grid AND region_coords_x = :region_coords_x AND region_coords_y = :region_coords_y";
        let is_sames = self.conn.exec_map(
            SQL_SELECT,
            params! { grid, region_coords_x, region_coords_y },
            |(size_x, size_y, samples_x, samples_y, scale, offset, elevs, name, water_level) : (u32, u32, u32, u32, f32, f32, Vec<u8>, String, f32)| {
                //  Is the stored data identical to what we just read from the region?
                log::trace!("Elevs:\n{:?} vs\n{:?}", elevs, new_elevs); // ***TEMP***
                let is_same = 
                    size_x == region_info.get_size()[0] && 
                    size_y == region_info.get_size()[1] &&
                    samples_x == samples[0] && 
                    samples_y == samples[1] &&
                    scale == region_info.scale &&
                    offset == region_info.offset &&
                    Self::check_elev_err_within_tolerance(&elevs, &new_elevs, scale, offset, Self::ELEV_ERROR_TOLERANCE) &&
                    name == region_info.name &&
                    water_level == region_info.water_lev;                    
                is_same
            },
        )?;
        //  Changed?
        Ok(if is_sames.is_empty() {
            ChangeStatus::None
        } else {
            //  Must be 1, because of SELECT on unique key.
            assert!(is_sames.len() == 1);
            if is_sames[0] {
                ChangeStatus::NoChange
            } else {
                ChangeStatus::Changed
            }
        })
    }

    // ***NOT WORKING***
    fn do_sql_update(
        &mut self,
        region_info: UploadedRegionInfo,
        params: &HashMap<String, String>,
    ) -> Result<(), Error> {
        const SQL_INSERT: &str = r"INSERT INTO raw_terrain_heights (grid, region_coords_x, region_coords_y, size_x, size_y, name, scale, offset, elevs,  water_level, creator) 
            VALUES (:grid, :region_coords_x, :region_coords_y, :size_x, :size_y, :name, :scale, :offset, :elevs, :water_level, :creator)";
        //  ***NEED TO FIX THIS FOR Open Simulator***
        let creator = params
            .get(OWNER_NAME)
            .ok_or_else(|| anyhow!("This request is not from Second Life/Open Simulator"))?
            .trim();
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
    fn parse_request(
        b: &[u8],
        _env: &HashMap<String, String>,
    ) -> Result<UploadedRegionInfo, Error> {
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
    fn process_request(
        &mut self,
        region_info: UploadedRegionInfo,
        params: &HashMap<String, String>,
    ) -> Result<String, Error> {
        let msg = format!("Region info:\n{:?}", region_info);
        let changed_status = self.do_sql_unchanged_check(&region_info, params)?;
        log::warn!("Changed status for region {}: {:?}", region_info.name, changed_status);
        //  Initial test of SQL
        self.do_sql_insert(&region_info, params)?; // ***TEMP***

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
                let params = request
                    .params
                    .as_ref()
                    .ok_or_else(|| anyhow!("No HTTP parameters found"))?;
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
                        let http_response = Response::http_response(
                            "text/plain",
                            500,
                            format!("Problem processing request: {:?}", e).as_str(),
                        );
                        Response::write_response(out, request, http_response.as_slice(), &[])?;
                    }
                }
            }
            Err(e) => {
                let http_response = Response::http_response(
                    "text/plain",
                    400,
                    format!("Incorrect request: {:?}", e).as_str(),
                );
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
    let mut terrain_upload_handler = TerrainUploadHandler::new(pool)?;
    //  Run the FCGI server.
    common::run(&mut instream, &mut outio, &mut terrain_upload_handler)
}

/// Main program
pub fn main() {
    logger();
    match run_responder() {
        Ok(()) => {}
        Err(e) => {
            log::error!("Upload server failed: {:?}", e);
            panic!("Upload server failed: {:?}", e);
        }
    }
}

#[test]
fn parse_terrain() {
    const TEST_JSON: &str = "{\"grid\":\"agni\",\"name\":\"Vallone\",\"scale\":1.092822,\"offset\":33.500740,\"water_lev\":20.000000,\"region_coords\":[1807,1199],\"elevs\":[\"E7CAACA3A5A8ACAEB0B2B5B9BDC0C4C5C5C3C0BDB9B6B3B2B2B3B4B7BBBFC3C7CBCED1D3\"]}";
    println!("TEST_JSON: {}", TEST_JSON);
    let parsed = UploadedRegionInfo::parse(TEST_JSON).expect("JSON misparsed");
    println!("Parsed JSON: {:?}", parsed);
    println!("Elevs: {:?}", parsed.get_unscaled_elevs());
}
