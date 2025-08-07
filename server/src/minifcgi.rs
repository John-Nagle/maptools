//! Mini FCGI framework.
//!
//! Used by FCGI applications (responders)
//! called from Apache-type web servers.
//! 
//! Reads from standard input, outputs to
//! standard output.
//!
//!
//! Normal usage:
//!
//!    pub fn main() {
//!        minifcgi::main(|_|{}, handler)
//!    }
//!
//  Animats
//  August, 2025
//
use anyhow::{Result};
use std::io;
use std::collections::{HashMap};


/// Request to server.
pub struct Request {
}

impl Request {
    /// New - reads a request from standard input.
    /// Can fail
    pub fn new() -> Result<Request> {
        todo!();
    }
}

/// Not the main program, but the main loop.
pub fn main(handler: fn(request: &Request, env: &HashMap<String, String>) -> Result<i32>) -> Result<i32> {
    let env = std::env::vars().map(|(k,v)| (k,v)).collect();
    loop {
        let request = Request::new()?;
        handler(&request, &env)?;
    }
}



//////fn handler(request: Request, env: HashMap<String, String>) ->Result<i32> 

//////fn handler(io: &mut dyn IO, env: HashMap<String, String>) -> Result<i32> {
