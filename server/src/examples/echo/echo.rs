//! FCGI echo server. 
//! For test use.
use std::io::Write;
use std::collections::HashMap;
use std::io::BufReader;
use minifcgi;
use minifcgi::{Request, Response};
use anyhow::{Error};
use log::LevelFilter;

/*
use std::collections::HashMap;

fn handler(io: &mut dyn IO, env: HashMap<String, String>) -> anyhow::Result<i32> {
    let mut all_data = Vec::new();
    let sink = io.read_to_end(&mut all_data)?;
    io.write_all(
        format!(
            r#"Content-type: text/plain; charset=utf-8

Hello World! Your request method was "{}"!
"#,
            env.get("REQUEST_METHOD").unwrap()
        )
        .as_bytes(),
    )?;
    Ok(0)
}
*/

/// Debug logging
fn logger() {
    //  Log file is openly visible as a web page.
    //  Only for debug tests.
    const LOG_FILE_NAME: &str = "logs/echolog.txt";
    let _ = simplelog::CombinedLogger::init(vec![
            simplelog::WriteLogger::new(
                LevelFilter::Info,
                simplelog::Config::default(),
                std::fs::File::create(LOG_FILE_NAME).expect("Unable to create log file"),
            ),
        ]);
    log::warn!("Logging to {:?}", LOG_FILE_NAME); // where the log is going
}


/// Handler. actually handles the FCGI request.
fn handler(out: &mut dyn Write, request: &Request, env: &HashMap<String, String>) -> Result<(), Error> {
    let http_response = Response::http_response("text/plain", 200, "OK");  
    //  Return something useful.
    let b = format!("Env: {:?}\nParams: {:?}", env, request.params).into_bytes();
    Response::write_response(out, request, http_response.as_slice(), &b)?;
    Ok(())
}

pub fn main() {
    logger();   // start logging
    let mut outio = std::io::stdout();
    /*  DUMMY
        outio.write_all(
        format!(
            r#"Content-type: text/plain; charset=utf-8

Hello World!
"#
        )
        .as_bytes(),
    ).expect("Dummy response");
    */
    //
    //////eprintln!("Starting FCGI"); // ***TEMP***
    let inio = std::io::stdin();
    let mut instream = BufReader::new(inio);
    minifcgi::run(&mut instream, &mut outio, handler).expect("Run failed");
}
