//! Upload Second Life / Open Simulator asset info to server
//! Part of the Animats impostor system
//!
//! At this point, the asset exists on the SL/OS asset store.
//! A script running in an SL/OS viewer calls this service to tell it about new assets.
//!
//!     License: LGPL.
//!     Animats
//!     August, 2025.
//
#![forbid(unsafe_code)]
use anyhow::{Error, anyhow};
use log::LevelFilter;
use common::Credentials;
use common::init_fcgi;
use common::{Handler, Request, Response};
use mysql::prelude::{Queryable};
use mysql::{Pool, TxOpts, PooledConn, params};
use std::collections::{HashMap};
use std::io::Write;
use common::{Authorizer, AuthorizeType};
use common::InitialImpostors;
use common::{AssetUpload, AssetUploadArrayShort};

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

/// Debug logging
fn logger() {
    //  Log file is openly visible as a web page.
    //  Only for debug tests.
    const LOG_FILE_NAME: &str = "logs/uploadimpostorlog.txt";
    let _ = simplelog::CombinedLogger::init(vec![simplelog::WriteLogger::new(
        LevelFilter::Debug,
        simplelog::Config::default(),
        ////std::fs::File::create(LOG_FILE_NAME).expect("Unable to create log file"),
        std::fs::OpenOptions::new().create(true).append(true).open(LOG_FILE_NAME).expect("Unable to create log file"),
    )]);
    log::warn!("Logging to {:?}", LOG_FILE_NAME); // where the log is going
}

///  Our handler
struct AssetUploadHandler {
    /// MySQL onnection pool. We only use one.
    #[allow(dead_code)] // needed to keep the pool alive, but never referenced.
    pool: Pool,
    /// Active MySQL connection.
    conn: PooledConn,
    /// Owner of object at other end
    owner_name: Option<String>,
}
impl AssetUploadHandler {

    /// Usual new. Saves connection pool for use.
    pub fn new(pool: Pool) -> Result<Self, Error> {
        let conn = pool.get_conn()?;
        Ok(Self { pool, conn, owner_name: None  })
    }

    /// Parse a request
    fn parse_request(
        b: &[u8],
        _env: &HashMap<String, String>,
    ) -> Result<AssetUploadArrayShort, Error> {
        //  Should be UTF-8. Check.
        let s = core::str::from_utf8(b)?;
        if s.trim().is_empty() {
            return Err(anyhow!("Empty request. JSON was expected"));
        }
        log::info!("Uploaded JSON:\n{}", s);
        //  Should be valid JSON
        let parsed: AssetUploadArrayShort = serde_json::from_str(s)?;
        Ok(parsed)
    }

    /// Handle request.
    ///
    /// Start a database transaction.
    /// Check if this data is the same as any stored data for this region.
    /// If yes, just update confirmation user and time.
    /// If no, replace old data entirely.
    fn process_request(
        &mut self,
        asset_info_short: AssetUploadArrayShort,
        _params: &HashMap<String, String>,
    ) -> Result<(usize, String), Error> {
        //  We have an array of assets.
        log::info!("Processing {} assets.", asset_info_short.len());
        //  An empty list means it's time to check to see if we're done and report errors.
        for asset_upload_short in &asset_info_short {
            let mut asset_upload = AssetUpload::new_from_asset_upload_short(asset_upload_short)?;
            log::debug!("Updating tile: {:?}", asset_upload);
            asset_upload.update_tile(&mut self.conn)?;
            log::debug!("Inserting UUID in initial_impostors: {:?}", asset_upload); 
            //  Tile asset updated. Now update initial impostors.
            if !InitialImpostors::insert_uuid(&mut self.conn, &asset_upload)? {
                //////.with_context(|| format!("Insert uuid failed for {:?}", asset_upload))? {
                log::debug!("Upload had no effect on impostors: {:?}", asset_upload);
            }
        }
        Ok((200, "Asset upload successful".to_string()))
    }
    
    /// Process a finished grid.
    /// LSL program sends this request after uploading everything.
    fn process_finish_grid(&mut self, grid: &str) ->  Result<(), Error> {
        log::info!("Grid_finished, ready to process: \"{}\"", grid);
        //  First check to see if uploads are complete.
        let unfinished_tiles = InitialImpostors::find_missing_uuids(&mut self.conn, grid)?;
        for tile in &unfinished_tiles {
            log::debug!("Unfinished tile: {:?}", tile);
        }
        println!("{} unfinished tiles.", unfinished_tiles.len());
        if unfinished_tiles.is_empty() {
            self.deploy_impostors(grid)?;
            log::info!("New impostors deployed.");
        } else {
            return Err(anyhow!("{} tiles still need to be uploaded: {:?}", unfinished_tiles.len(), unfinished_tiles));
        }
        Ok(())
    }
    
    /// Deploy impostors by copying all entries from this grid from initial_impostors to region_impostors table.
    fn deploy_impostors(&mut self, grid: &str) -> Result<(), Error> {
        log::info!("Deploying impostors for {}", grid);
        const SQL_DELETE_GRID: &str = r"DELETE FROM region_impostors WHERE grid = :grid";
        const SQL_COPY_GRID: &str = r"INSERT INTO region_impostors SELECT * FROM initial_impostors WHERE grid = :grid";
        let params = params! {
            "grid" => grid
        };
        //  Atomic transaction. Must complete successfully or rolled back.
        let mut tx = self.conn.start_transaction(TxOpts::default())?;
        log::debug!("Deleting old.");
        tx.exec_drop(SQL_DELETE_GRID, &params)?;
        log::debug!("Inserting new.");
        tx.exec_drop(SQL_COPY_GRID, &params)?;
        log::debug!("Deploy complete.");
        Ok(tx.commit()?)
    }
    
    /// Internal handler. Caller sends HTTP response.
    fn handler_internal(
        &mut self,
        _out: &mut dyn Write,
        request: &Request,
        env: &HashMap<String, String> ,
    ) -> Result<(), Error> {
        //  Process params and authorization
        let params = request
            .params
            .as_ref()
            .ok_or_else(|| anyhow!("No HTTP parameters found"))?;
             //  This must be a POST
             if let Some(request_method) = params.get("REQUEST_METHOD") {  
                if request_method.to_uppercase().trim() != "POST" {             
                    return Err(anyhow!("Request method \"{}\" was not POST.", request_method));
            }
        } else {
            return Err(anyhow!("No HTTP request method."));
        };
        //  Authorize
        self.owner_name = Some(Authorizer::authorize(AuthorizeType::UploadImpostors, env, params)?);
        //  Check params from URL
        //  Presence of ?grid_finished=gridname triggers this.
        let query_string = params.get("QUERY_STRING").ok_or_else(|| anyhow!("No QUERY_STRING from FCGI"))?;
        let query_params = querystring::querify(query_string);
        let query_params_map: HashMap<&str, &str> = query_params.into_iter().collect();
        if let Some(grid) = query_params_map.get("grid_finished") {
            //  Client says all done. Try to process.
            log::info!("Final grid_finished request for {}", grid);
            self.process_finish_grid(grid)?;
        } else {
            //  Main path - this is data about assets just uploaded.
            log::info!("Impostor upload request");
            let req = Self::parse_request(&request.standard_input, env)?;
            self.process_request(req, params)?;
        }
        Ok(())
    }
}
//  Our "handler"
impl Handler for AssetUploadHandler {
    fn handler(
        &mut self,
        out: &mut dyn Write,
        request: &Request,
        env: &HashMap<String, String>,
    ) -> Result<(), Error> {
        //  Process params and authorization
        log::info!("============ New request made ==================");
        match self.handler_internal(out, request, env) {
            Ok(_) => {
                //  Success. Send a plain "OK"
                let http_response = Response::http_response("text/plain", 200, "OK");
                //  Return something useful.
                let b = "Done".as_bytes();
                Response::write_response(out, request, http_response.as_slice(), b)?;
            }
            Err(e) => {
                let http_response = Response::http_response("text/plain", 500, "Error");
                let s = format!("Problem processing request: {:?}", e);
                let b = s.as_bytes();
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
    let pool = Pool::new(opts)?;
    log::info!("Connected to database.");
    let mut asset_upload_handler = AssetUploadHandler::new(pool)?;
    //  Run the FCGI server.
    common::run(&mut instream, &mut outio, &mut asset_upload_handler)
}

/// Main program
pub fn main() {
    logger();
    // Set a custom panic hook
    std::panic::set_hook(Box::new(|info| {
        log::error!("PANIC: {:?}", info);
    }));
    match run_responder() {
        Ok(()) => {}
        Err(e) => {
            log::error!("Upload server failed: {:?}", e);
            panic!("Upload server failed: {:?}", e);
        }
    }
}

