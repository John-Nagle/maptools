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
use std::io::{Read, Write, BufRead, Stdin, Stdout};
use std::io::{BufReader, BufWriter};

/// Wraps the stdin and stdout streams of a standard CGI invocation.
///
/// See the [Common Gateway Interface][1] specification for more information.
///
/// [1]: https://tools.ietf.org/html/rfc3875
///
/// This is from outer_cgi from crates.io.
///
pub trait IO : BufRead + Write {
}

struct DualIO<R: BufRead, W: Write> {
    i: R,
    o: W,
}

impl<R: BufRead, W: Write> Read for DualIO<R, W> {
    fn read(&mut self, buf: &mut[u8]) -> io::Result<usize> {
        self.i.read(buf)
    }
}

impl<R: BufRead, W: Write> BufRead for DualIO<R, W> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.i.fill_buf()
    }
    fn consume(&mut self, amount: usize) {
        self.i.consume(amount)
    }
}

impl<R: BufRead, W: Write> Write for DualIO<R, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.o.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.o.flush()
    }
}

impl<R: BufRead, W: Write> IO for DualIO<R, W> {
}


/// Request to server.
pub struct Request {
}

impl Request {
    /// New - reads a request from standard input.
    /// Can fail
    pub fn new<R: Read>(io: &DualIO<BufReader<R>, Stdout>) -> Result<Request> {
        todo!();
    }
}

/// Not the main program, but the main loop.
pub fn main(handler: fn(request: &Request, env: &HashMap<String, String>) -> Result<i32>) -> Result<i32> {
    let env = std::env::vars().map(|(k,v)| (k,v)).collect();
    let dual_io = DualIO { i: BufReader::new(io::stdin()), o: io::stdout()};
    loop {
        let request = Request::new(&dual_io)?;
        handler(&request, &env)?;
    }
}



//////fn handler(request: Request, env: HashMap<String, String>) ->Result<i32> 

//////fn handler(io: &mut dyn IO, env: HashMap<String, String>) -> Result<i32> {
