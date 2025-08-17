//! FCGI echo server. 
//! For test use.
use std::io::Write;
use std::collections::HashMap;
//////use std::io::BufReader;
use minifcgi;
use minifcgi::{Request, Response};
use minifcgi::{init_fcgi};
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
                LevelFilter::Debug,
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

    log::warn!("stdin points to {}", std::fs::read_link("/proc/self/fd/0").unwrap().display());
    log::warn!("Environment: {:?}", std::env::vars());
    use std::os::unix::net::UnixListener;
    //////let stdin = std::io::stdin();
    //////drop(stdin);
    
    //////let listener = match UnixListener::bind("/proc/self/fd/0") {
    let listener = match init_fcgi() {
        Ok(listener) => {
            log::info!("Bound to listener: {:?}", listener);
            listener
        }
        Err(e) => {
            log::error!("bind function failed: {e:?}");
            panic!("Can't open");
        }
    };

    //////let listener = stdin;
/*
    use std::os::fd::FromRawFd;
    let mut listener = None;
    unsafe { // ***AARGH***
        listener = Some(UnixListener::from_raw_fd(0));
    };
    let listener = listener.unwrap();
*/
    let socket = match listener.accept() {
        Ok((socket, addr)) => {
            log::info!("Got a client: {addr:?}");
            socket }
        Err(e) => {
            log::error!("accept function failed: {e:?}");
            panic!("Can't open");
        }
    };
    let outsocket = socket.try_clone().expect("Unable to clone socket");
    let mut instream = std::io::BufReader::new(socket);
    let mut outio = std::io::BufWriter::new(outsocket); 
    //////let inio = std::io::stdin();
    //////inio.set_raw_mode().unwrap();
    //////let mut instream = BufReader::new(inio);
    //////let mut instream = inio.lock(); // Lock the stdin for reading.
    /*
    // ***TEMP TEST***
    let mut header_bytes:[u8;8] = Default::default();
    use std::io::Read;
    let stat = instream.read_exact(&mut header_bytes);
    log::debug!("Stat: {:?} Bytes: {:?}", stat, header_bytes);
    std::process::exit(0);
    // ***END TEMP***
    */
    minifcgi::run(&mut instream, &mut outio, handler).expect("Run failed");
}
