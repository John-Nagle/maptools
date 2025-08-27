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
    coords: [u32;2],
    /// Region size. 256 x 256 if ommitted.
    size: Option<[u32;2]>,
    /// Region name
    name: String,
    /// Height data, a long set of hex data.  
    elevs: String,
    //  Water level
    water_lev: f32,
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
    fn parse_request(b: &[u8], env: &HashMap<String, String>) -> Result<UploadedRegionInfo, Error> {
        //  Should be UTF-8. Check.
        let s = core::str::from_utf8(b)?;
        //  Should be valid JSON
        Ok(serde_json::from_str(s)?)        
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
    const TEST_JSON: str = "{\"region\":\"Vallone\",\"scale\":1.092822,\"offset\":33.500740,\"waterlev\":20.000000,\"regioncoords\":[1807,1199],\"elevs\":[\"E7CAACA3A5A8ACAEB0B2B5B9BDC0C4C5C5C3C0BDB9B6B3B2B2B3B4B7BBBFC3C7CBCED1D3D5D5D4CFC4B5A4\"";
}
