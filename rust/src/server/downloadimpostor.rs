//! Download Second Life / Open Simulator terrain info from
//! Part of the Animats impostor system
//!
//! Viewers call this to get information about impostors.
//! 
//! Requests:
//!
//!     https://animatsinfo/actions/downloadimpostor.fcgi?grid=NAME,x=NNN,y=NNN
//!
//! Returns info for one region, if available.
//!
//!     https://animatsinfo/actions/downloadimpostor.fcgi?grid=NAME, viz_group=NNN
//!
//! Returns info for one visibility group, if available.
//!
//!     https://animatsinfo/actions/downloadimpostor.fcgi?grid=NAME
//!
//! Returns info for an entire grid. Mostly for test purposes.
//!
//! Data is returned as JSON. Format is currently on animats.com.
//! There is no authentication. Anyone can read this data.
//!
//!     License: LGPL.
//!     Animats
//!     October, 2025.
//
#![forbid(unsafe_code)]
use anyhow::{Error, anyhow};
use log::LevelFilter;
use uuid::Uuid;
use common::Credentials;
use common::init_fcgi;
use common::{Handler, Request, Response};
use common::{UploadedRegionInfo};
use common::{RegionImpostorData, RegionImpostorLod};
use common::u8_to_elev;
use mysql::prelude::{Queryable};
use mysql::{Pool};
use mysql::{PooledConn, params, Row};
use std::collections::HashMap;
use std::io::Write;

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
const DOWNLOAD_CREDS_FILE: &str = "download_credentials.txt";

/// Debug logging
fn logger() {
    //  Log file is openly visible as a web page.
    //  Only for debug tests.
    const LOG_FILE_NAME: &str = "logs/downloadlog.txt";
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
struct TerrainDownloadHandler {
    /// MySQL onnection pool. We only use one.
    pool: Pool,
    /// Active MySQL connection.
    conn: PooledConn,
    /// Owner of object at other end
    owner_name: Option<String>,
}
impl TerrainDownloadHandler {

    /// Usual new. Saves connection pool for use.
    pub fn new(pool: Pool) -> Result<Self, Error> {
        let conn = pool.get_conn()?;
        Ok(Self { pool, conn, owner_name: None  })
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
    
    /// Build the SQL query statement.
    fn build_sql_query(params: &HashMap<String, String>) -> Result<(String, String, Option<(u32, u32)>, Option<u32>), Error> {
        //  Parse URL parameters.  Build WHILE part.
        let query_string = params.get("QUERY_STRING").ok_or_else(|| anyhow!("No QUERY_STRING from FCGI"))?;
        let query_vec = querystring::querify(query_string);
        let query_params: HashMap<String, String> = query_vec.iter().map(|(k, v)| (k.to_lowercase().trim().to_string(), v.to_string())).collect();
        //  Parameters are
        //      grid
        //      x
        //      y
        //      viz_group
        //  Grid is mandatory, others are optional.
        let grid = query_params.get("grid").ok_or_else(|| anyhow!("No \"grid\" parameter in HTTP request"))?;
        let coords_opt: Option<(u32, u32)> = {
            if let Some(x) = query_params.get("x") {            
                if let Some(y) = query_params.get("y") {
                    Some((x.parse()?, y.parse()?))
                } else {
                    None
                }
            } else {
                None
            }
        };
        let viz_group_opt: Option<u32> = if let Some(vg) = query_params.get("viz_group") {
            Some(vg.parse()?)
        } else {
            None
        };
        
        //  There are three cases.
        let where_clause = if viz_group_opt.is_some() {
            "grid = : grid AND viz_group = : viz_group"
        } else if coords_opt.is_some() {
            "grid = : grid AND region_loc_x = :region_loc_x AND region_loc_y = : region_loc_y"
        }
        else {
            ""   
        };
        log::info!("Query: grid: {} coords {:?}  viz_group: {:?}, WHERE clause: {}", grid, coords_opt, viz_group_opt, where_clause);
        const SELECT_PART: &str = "grid, region_loc_x, region_loc_y, name, region_size_x, region_size_y, scale_x, scale_y, scale_z, \
        elevation_offset, impostor_lod, viz_group, mesh_uuid, sculpt_uuid, water_height, creator, creation_time, faces_json FROM region_impostors ";
        let priority = if where_clause.is_empty() { " LOW PRIORITY ". to_string() } else { "".to_string() };
        let stmt = format!("SELECT {}{} WHERE {} ORDER BY grid, region_loc_x, region_loc_y", SELECT_PART, priority, where_clause);
        Ok((stmt, grid.clone(), coords_opt, viz_group_opt))
    }
    
    /// Select the desired items and generate JSON.
    fn do_select(&mut self, params: &HashMap<String, String>) -> Result<(), Error> {
        //  Convert UUIDs, return None if fail.
        fn convert_uuid(s_opt: Option<String>) -> Option<Uuid> {
            if let Some(s) = s_opt {
                match Uuid::try_parse(&s) {
                    Ok(u) => Some(u),
                    Err(_) => None
                }
            } else {
                None
            }
        }
        // Build SELECT statement and get params
        let (stmt, grid, coords_opt, viz_group_opt) = Self::build_sql_query(params)?;
        let viz_group = if let Some(viz_group) = viz_group_opt { viz_group } else { 0 };
        let (region_coords_x, region_coords_y) = if let Some(coords) = coords_opt { (coords.0, coords.1) } else { (0, 0) };
        //  Perform the SELECT
        let mut query_result: mysql::QueryResult<_> = self.conn.exec_iter(
            stmt,
            params! { grid, region_coords_x, region_coords_y, viz_group })?;
        //  Process the results.
        //  There should be only one query result set since we only made one query.
        //  So this is iteration over rows.
        let first_result_set: mysql::ResultSet<_> = query_result.iter().expect("No result set from SELECT");
        let error_sink: Result<(), Error> = first_result_set.map(|rs: Result<mysql::Row, mysql::Error> | {          
            log::debug!("SELECT result: {:?}", rs);    // ***TEMP***
            let row = rs?;
            let rd = RegionImpostorData {
                grid: row.get_opt(0).ok_or_else(|| anyhow!("grid is null"))??,
                region_loc: [row.get_opt(1).ok_or_else(|| anyhow!("loc_x is null"))??, row.get_opt(2).ok_or_else(|| anyhow!("loc_y is null"))??],
                name: row.get_opt(3).ok_or_else(|| anyhow!("name is null"))??,
                region_size: [row.get_opt(4).ok_or_else(|| anyhow!("size_x is null"))??, row.get_opt(5).ok_or_else(|| anyhow!("size_y is null"))??],
                scale: [
                    row.get_opt::<u32, _>(6).ok_or_else(|| anyhow!("scale_x is null"))?? as f32, 
                    row.get_opt::<u32, _>(7).ok_or_else(|| anyhow!("scale_y is null"))?? as f32, 
                    row.get_opt(8).ok_or_else(|| anyhow!("scale_z is null"))??],
                elevation_offset: row.get_opt(9).ok_or_else(|| anyhow!("elevation_offset is null"))??,
                impostor_lod: row.get_opt(10).ok_or_else(|| anyhow!("impostor_lod is null"))??,
                viz_group: row.get_opt(11).ok_or_else(|| anyhow!("Viz_group is null"))??,
                mesh_uuid: convert_uuid(row.get_opt(12).ok_or_else(|| anyhow!("mesh_uuid is null"))??,),
                sculpt_uuid: convert_uuid(row.get_opt(13).ok_or_else(|| anyhow!("mesh_uuid is null"))??,),
                water_height: row.get_opt(14).ok_or_else(|| anyhow!("water_height is null"))??,
                faces: row.get_opt(17).ok_or_else(|| anyhow!("faces_json is null"))??, // ***TEMP***
            };
            log::debug!("{:?}",rd);
            Ok(())
        }).collect();
        error_sink?;
        Ok(())
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
    ) -> Result<(usize, String), Error> {
        self.do_select(params)?;
        //  ***MORE*** output JSON
        Ok((200, "Done".to_string()))
    }
}
//  Our "handler"
impl Handler for TerrainDownloadHandler {
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
                //  This must be a GET
                if let Some(request_method) = params.get("REQUEST_METHOD") {   
                    if request_method.to_uppercase().trim() != "GET" {             
                        return Err(anyhow!("Request method \"{}\" was not GET.", request_method));
                    }            
                } else {
                    return Err(anyhow!("No HTTP request method."));
                }
                //  Process. Error 500 if fail.
                match self.process_request(req, &params) {
                    Ok((status, msg)) => {
                        //  Success. Send a plain "OK"
                        let http_response = Response::http_response("text/plain", status, "OK");
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
    let creds = Credentials::new(DOWNLOAD_CREDS_FILE)?;
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
    let mut terrain_upload_handler = TerrainDownloadHandler::new(pool)?;
    //  Run the FCGI server.
    common::run(&mut instream, &mut outio, &mut terrain_upload_handler)
}

/// Main program
pub fn main() {
    logger();
    match run_responder() {
        Ok(()) => {}
        Err(e) => {
            log::error!("Download server failed: {:?}", e);
            panic!("Download server failed: {:?}", e);
        }
    }
}

