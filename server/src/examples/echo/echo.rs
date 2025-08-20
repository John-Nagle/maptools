//! FCGI echo server.
//! For test use.
use std::collections::HashMap;
use std::io::Write;
//////use std::io::BufReader;
use anyhow::Error;
use log::LevelFilter;
use minifcgi;
use minifcgi::init_fcgi;
use minifcgi::{Request, Response, Handler};

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
struct EchoHandler {
        cnt: usize
    }
impl EchoHandler {
    pub fn new() -> Self {
        Self {
            cnt: 0
        }
    }
}
//  Our "handler"
impl Handler for EchoHandler {
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

/*
/// Handler. actually handles each FCGI request.
fn handler(
    out: &mut dyn Write,
    request: &Request,
    env: &HashMap<String, String>,
    user_params: &Box<&dyn Any>,
) -> Result<(), Error> {
    //  Access user params.
    let tally = {   let uprefcell  = user_params.downcast_ref::<RefCell<HandlerParams>>().unwrap();
        let mut up = uprefcell.borrow_mut();    // get access to innards
        up.n += 1;  // tally
        up.n
    };
    let http_response = Response::http_response("text/plain", 200, "OK");
    //  Return something useful.
    let b = format!("Env: {:?}\nParams: {:?}\ntally: {}", env, request.params, tally).into_bytes();
    Response::write_response(out, request, http_response.as_slice(), &b)?;
    Ok(())
}
*/

/// Main program
pub fn main() {
    logger(); // start logging
    log::info!(
        "stdin points to {}",
        std::fs::read_link("/proc/self/fd/0").unwrap().display()
    );
    log::info!("Environment: {:?}", std::env::vars());
    //  Set up in and out sockets.
    //  Communication with the parent process is via a UNIX socket.
    //  This is a pain to set up, because UNIX sockets are badly mis-matched
    //  to parent/child process communication.
    //  See init_fcgi for how it is done.
    let listener = match init_fcgi() {
        Ok(listener) => {
            log::info!("init_fcgi created listener: {:?}", listener);
            listener
        }
        Err(e) => {
            log::error!("init_fcgi was unable to create listener: {e:?}");
            panic!("Can't open");
        }
    };
    //  Accept a connection on the listener socket. This hooks up
    //  input and output to the parent process.
    let socket = match listener.accept() {
        Ok((socket, _addr)) => socket,
        Err(e) => {
            log::error!("accept connection from parent process failed: {e:?}");
            panic!("accept connection from parent process failed");
        }
    };
    let outsocket = socket.try_clone().expect("Unable to clone socket");
    let mut instream = std::io::BufReader::new(socket);
    let mut outio = std::io::BufWriter::new(outsocket);
    //  Dummy user data
    let mut echo_handler = EchoHandler::new();
    //  Run the FCGI server.
    minifcgi::run(&mut instream, &mut outio, &mut echo_handler).expect("Run failed");
}
