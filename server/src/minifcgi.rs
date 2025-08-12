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
use anyhow::{Result, Error, anyhow};
use std::io;
use std::collections::{HashMap};
use std::io::{Read, Write, BufRead, Stdin, Stdout};
use std::io::{BufReader, BufWriter};
#[macro_use]
use num_derive::{FromPrimitive}; // Derive the FromPrimitive trait
use num_traits::{FromPrimitive};

/// Wraps the stdin and stdout streams of a standard CGI invocation.
///
/// See the [Common Gateway Interface][1] specification for more information.
///
/// [1]: https://tools.ietf.org/html/rfc3875
///
/// This is from outer_cgi from crates.io.
///
/// All this generic complexity is so we can test this thing
/// using something other than stdin/stdout.
///
/// Protocol: see https://cs.opensource.google/go/go/+/master:src/net/http/fcgi/fcgi.go
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

/*
The Go version

// recType is a record type, as defined by
// https://web.archive.org/web/20150420080736/http://www.fastcgi.com/drupal/node/6?q=node/22#S8
type recType uint8

const (
	typeBeginRequest    recType = 1
	typeAbortRequest    recType = 2
	typeEndRequest      recType = 3
	typeParams          recType = 4
	typeStdin           recType = 5
	typeStdout          recType = 6
	typeStderr          recType = 7
	typeData            recType = 8
	typeGetValues       recType = 9
	typeGetValuesResult recType = 10
	typeUnknownType     recType = 11
)

// keep the connection between web-server and responder open after request
const flagKeepConn = 1

const (
	maxWrite = 65535 // maximum record body
	maxPad   = 255
)

const (
	roleResponder = iota + 1 // only Responders are implemented.
	roleAuthorizer
	roleFilter
)

const (
	statusRequestComplete = iota
	statusCantMultiplex
	statusOverloaded
	statusUnknownRole
)

type header struct {
	Version       uint8
	Type          recType
	Id            uint16
	ContentLength uint16
	PaddingLength uint8
	Reserved      uint8
}
*/
/// Type of FCGI record. Almost always BeginRequest.
#[derive(Debug, FromPrimitive)]
enum FcgiRecType {
	BeginRequest = 1,
	AbortRequest = 2,
    EndRequest = 3,
	Params = 4,
	Stdin = 5,
	Stdout = 6,
	Stderr = 7,
	Data = 8,
	GetValues = 9,
	GetValuesResult = 10,
	UnknownType = 11,
}

/// FCGI header record, deserialized.
pub struct FcgiHeader {
	version: u8,
	/// Record type. Usually BeginRequest.
	rec_type: FcgiRecType,
	/// Request ID
	id: u16,
	/// Length of content, in bytes.
	content_length:  u16,
	/// Padding. Read content_length + padding.
	padding_length: u8,
	/// For unlikely future extension.
	reserved: u8,
}

impl FcgiHeader {
    /// Deserialize 8 bytes to an FCGI header.
    fn new_from_bytes(b: &[u8;8]) -> Result<FcgiHeader, Error> {
        let content_length = u16::from_be_bytes(<[u8;2]>::try_from(&b[4..6]).unwrap());
        let padding_length = 8 - u8::try_from(content_length & 7).unwrap();  // padding needed to round up to next multiple of 8 ***CHECK THIS***
        Ok(
            FcgiHeader {
                version: b[0],
                rec_type: FcgiRecType::from_u8(b[1]).ok_or_else(|| anyhow!("Invalid FCGI record type: {}", b[1]))?,
                id: u16::from_be_bytes(<[u8;2]>::try_from(&b[2..4]).unwrap()),
                content_length,
                //  h.PaddingLength = uint8(-contentLength & 7)  -- go version
                padding_length, 
                reserved: b[7],
            }
        )     
    }
}

/// Request to server.
#[derive (Debug)]
pub struct Request {
}

impl Request {
    /// New - reads a request from standard input.
    /// Can fail
    pub fn new() -> Result<Request> {
        Ok(Request {

        })
    }
}

/// Not the main program, but the main loop.
pub fn run<R: BufRead, W: Write>(io: DualIO<R,W>, handler: fn(out: &dyn Write, request: &Request, env: &HashMap<String, String>) -> Result<i32>) -> Result<i32> {
    let env = std::env::vars().map(|(k,v)| (k,v)).collect();
    loop {
        let request = Request::new()?;
        handler(&io.o, &request, &env)?;
    }
}

#[test]
fn basic_io() {
    fn do_req<W: Write>(out: &dyn Write, request: &Request, env: &HashMap<String, String>) -> Result<i32> {
        Ok(200)   
    }
    let test_data: Vec<u8> = "ABCDEF".as_bytes().to_vec();
    //////let data: Vec<u8> = vec![1, 2, 3, 4, 5];
    let cursor = std::io::Cursor::new(test_data);
    //////let mut buf_reader = BufReader::new(cursor);
    //////let io = DualIO{i: BufReader::new(io::stdin()), o: io::stdout()};
    let io = DualIO{i: BufReader::new(cursor), o: io::stdout()};
    let final_result = run(io, do_req::<&Stdout>);
    println!("Final result: {:?}", final_result);
}


//////fn handler(request: Request, env: HashMap<String, String>) ->Result<i32> 

//////fn handler(io: &mut dyn IO, env: HashMap<String, String>) -> Result<i32> {
