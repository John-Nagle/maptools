//! Upload Second Life / Open Simulator terrain to server
//! Part of the Animats impostor system
//!
//!
//! A Second Life/Open Simulator LSL script records terrain heights when visiting
//! regions. It calls this FCGI responder to upload that data to a server.
//! Later processing turns that into objects viewable in world via the
//! region impostor system.
//
//  Animats
//  August, 2025.

use std::collections::HashMap;
use std::io::Write;
use anyhow::Error;
use log::LevelFilter;
use chrono::{NaiveDateTime, Utc};
use mysql::{OptsBuilder, Opts, Conn, Pool};
use minifcgi::init_fcgi;
use minifcgi::{Request, Response, Handler};
use minifcgi::Credentials;
use serde::{Deserialize};

/// MySQL Credentials for uploading.
/// This filename will be searched for in parent directories,
/// so it can be placed above the web root, where the web server can't see it.
const UPLOAD_CREDS_FILE: &str = "upload_credentials.txt";
/// Database name for terrain info
const DB_NAME: &str = "terrain";
/// Default region size, used on grids that don't do varregions.
const DEFAULT_REGION_SIZE: u32 = 256;

/// Debug logging
fn logger() {
    //  Log file is openly visible as a web page.
    //  Only for debug tests.
    const LOG_FILE_NAME: &str = "logs/echolog.txt";
    let _ = simplelog::CombinedLogger::init(vec![simplelog::WriteLogger::new(
        LevelFilter::Debug,
        simplelog::Config::default(),
        std::fs::File::create(LOG_FILE_NAME).expect("Unable to create log file"),
    )]);
    log::warn!("Logging to {:?}", LOG_FILE_NAME); // where the log is going
}
///  Our data as uploaded from SL/OS in JSON format
// "{\"region\":\"Vallone\",\"scale\":1.092822,\"offset\":33.500740,\"waterlev\":20.000000,\"regioncoords\":[1807,1199],
//  \"elevs\":[\"E7CAACA3A5A8ACAEB0B2B5B9BDC0C4C5C5C3C0BDB9B6B3B2B2B3B4B7BBBFC3C7CBCED1D3D5D5D4CFC4B5A4"";
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UploadedRegionInfo {
    /// Grid name
    grid: String,
    /// Position of region in world, meters.
    pub region_coords: [u32;2],
    /// Region size. 256 x 256 if ommitted.
    size: Option<[u32;2]>,
    /// Region name
    name: String,
    /// Height data, a long set of hex data.  
    elevs: String,
    /// Scale factor for elevs
    scale: f32,
    /// Offset factor for elevs
    /// actual = input*scale + offset
    offset: f32,
    //  Water level
    pub water_lev: f32,
}

impl UploadedRegionInfo {
    /// Parse from string
    pub fn parse(s: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(s)?)
    }
    
    /// Get size, applying default region size for non-varregions
    pub fn get_size(&self) -> [u32;2] {
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
    
    /// Get elevations as numbers before offsetting.
    /// Input is a hex string representing one elev per byte.
    pub fn get_unscaled_elevs(&self) -> Result<Vec<u8>, Error> {
        Ok(hex::decode(&self.elevs)?)
    }
    
    /// Scale the elevations
    pub fn get_scaled_elevs(&self) -> Result<Vec<f32>, Error> {
        Ok(self.get_unscaled_elevs()?.iter().map(|&v| ((v as f32) / 256.0) * self.scale + self.offset).collect())
    }
}
///  Our handler
struct TerrainUploadHandler {
    pool: Pool,
}
impl TerrainUploadHandler {
    /// Usual new. Saves connection pool for use.
    pub fn new(pool: Pool) -> Self {
        Self {
            pool,
        }
    }
    
    /// Parse a request
    fn parse_request(b: &[u8], _env: &HashMap<String, String>) -> Result<UploadedRegionInfo, Error> {
        //  Should be UTF-8. Check.
        let s = core::str::from_utf8(b)?;
        //  Should be valid JSON
        Ok(UploadedRegionInfo::parse(s)?)        
    }
    
    /// Handle request.
    /// 
    /// Start a database transaction.
    /// Check if this data is the same as any stored data for this region.
    /// If yes, just update confirmation user and time.
    /// If no, replace old data entirely.
    fn process_request(region_info: UploadedRegionInfo, env: &HashMap<String, String>) -> Result<(), Error> {
        Ok(())  // ***TEMP***
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
                //  Process. Error 500 if fail.
                match Self::process_request(req, env) {
                    Ok(_) => (),
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
        //  Dummy up a response
        let http_response = Response::http_response("text/plain", 200, "OK");
        //  Return something useful.
        let b = format!("Env: {:?}\nParams: {:?}\n", env, request.params).into_bytes();
        Response::write_response(out, request, http_response.as_slice(), &b)?;
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
    let opts = mysql::OptsBuilder::new()
        .user(creds.get("DB_USER"))
        .db_name(Some(DB_NAME));
    drop(creds);
    let pool = Pool::new(opts)?;
    //  Process terrain data
    let mut terrain_upload_handler = TerrainUploadHandler::new(pool);
    //  Run the FCGI server.
    minifcgi::run(&mut instream, &mut outio, &mut terrain_upload_handler)
}

/// Main program
pub fn main() {
    logger();
    match run_responder() {
        Ok(()) => {},
        Err(e) => {
            log::error!("Upload server failed: {:?}", e);
            panic!("Upload server failed: {:?}", e);
        }
    }
}

#[test]
fn parse_terrain() {
    const TEST_JSON: &str = "{\"grid\":\"agni\",\"name\":\"Vallone\",\"scale\":1.092822,\"offset\":33.500740,\"water_lev\":20.000000,\"region_coords\":[1807,1199],\"elevs\":\"E7CAACA3A5A8ACAEB0B2B5B9BDC0C4C5C5C3C0BDB9B6B3B2B2B3B4B7BBBFC3C7CBCED1D3\"}";
    println!("TEST_JSON: {}", TEST_JSON);
    let parsed = UploadedRegionInfo::parse(TEST_JSON).expect("JSON misparsed");
    println!("Parsed JSON: {:?}", parsed);
    println!("Elevs: {:?}", parsed.get_scaled_elevs());
}
