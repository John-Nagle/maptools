/*
use vintage as fcgi;
use fcgi::{Request, Response};
use std::collections::HashMap;
use std::sync::Arc;
use std::io::Write;
use std::fs;
use mysql::*;
use mysql::prelude::*;
mod eventlogger;  // <- put your previous translation in `eventlogger.rs`

use eventlogger::*;

fn extract_headers(params: &Params) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    // FCGI puts HTTP headers as HTTP_* env variables.
    for (k, v) in params.iter() {
        if k.starts_with("HTTP_") {
            let orig = k.strip_prefix("HTTP_").unwrap()
                .replace('_', "-")
                .to_ascii_titlecase();
            headers.insert(orig, v.clone());
        }
    }
    headers
}

fn main() {
    // Load config at startup
    let config = Arc::new(read_config("config.json").expect("Failed to load config"));
    let url = format!(
        "mysql://{}:{}@{}/{}",
        config.mysql.user, config.mysql.password, config.mysql.domain, config.mysql.database
    );
    let pool = Pool::new(url).expect("Failed to connect to MySQL");

    fastcgi::run(|mut req: Request| {
        let params = req.params().clone();
        let headers = extract_headers(&params);
        let config = config.clone();
        let pool = pool.clone();

        let mut body = Vec::new();
        req.read_to_end(&mut body).unwrap();

        let status = match add_event(&body, &headers, &config, &pool) {
            Ok(_) => {
                let mut resp = Response::new();
                resp.write_all(b"OK\n").unwrap();
                req.respond(resp).unwrap();
                0
            }
            Err(e) => {
                let mut resp = Response::new();
                resp.write_all(format!("Error: {}\n", e).as_bytes()).unwrap();
                req.respond(resp).unwrap();
                1
            }
        };
        Ok(())
    }).expect("FCGI server failed");
}
use std::io::Write;
use std::collections::HashMap;
use std::io::BufReader;
use minifcgi;
use minifcgi::{Request, Response};
use anyhow::{Error};
/// Handler. actually handles the FCGI request.
fn handler(out: &mut dyn Write, request: &Request, env: &HashMap<String, String>) -> Result<(), Error> {
    //  ***MORE***
    Ok(())
}

pub fn main() {
    let inio = std::io::stdin();
    let mut outio = std::io::stdout();
    let mut instream = BufReader::new(inio);
    minifcgi::run(&mut instream, &mut outio, handler);
}
*/
