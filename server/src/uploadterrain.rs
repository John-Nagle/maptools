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
use minifcgi::init_fcgi;
use minifcgi::{Request, Response, Handler};
use mysql::{OptsBuilder, Opts, Conn};
use minifcgi::Credentials;

/// MySQL Credentials for uploading.
/// This filename will be searched for in parent directories,
/// so it can be placed above the web root, where the web server can't see it.
const UPLOAD_CREDS_FILE: &str = "upload_credentials.txt";

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

//  Our data
struct TerrainUploadHandler {
        cnt: usize
}
impl TerrainUploadHandler {
    pub fn new() -> Self {
        Self {
            cnt: 0
        }
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
        // Dummy up a response
        self.cnt += 1;
        let http_response = Response::http_response("text/plain", 200, "OK");
        //  Return something useful.
        let b = format!("Env: {:?}\nParams: {:?}\ntally: {}", env, request.params, self.cnt).into_bytes();
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
    .user(Some("foo"))
    .db_name(Some("bar"));
    //  Process terrain data
    let mut terrain_upload_handler = TerrainUploadHandler::new();
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
