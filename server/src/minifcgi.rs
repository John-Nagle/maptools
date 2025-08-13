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
//!        minifcgi::run(|_|{}, handler)
//!    }
//!
// What a request and response looks like:
// 
//     {FCGI_BEGIN_REQUEST,   1, {FCGI_RESPONDER, 0}}
//     {FCGI_PARAMS,          1, "\013\002SERVER_PORT80\013\016SER"}
//     {FCGI_PARAMS,          1, "VER_ADDR199.170.183.42 ... "}
//     {FCGI_PARAMS,          1, ""}
//     {FCGI_STDIN,           1, "quantity=100&item=3047936"}
//     {FCGI_STDIN,           1, ""}
// 
//         {FCGI_STDOUT,      1, "Content-type: text/html\r\n\r\n<html>\n<head> ... "}
//         {FCGI_STDOUT,      1, ""}
//         {FCGI_END_REQUEST, 1, {0, FCGI_REQUEST_COMPLETE}}
//
// Ref: https://www.mit.edu/~yandros/doc/specs/fcgi-spec.html
//!
//! Since this code is intended to support only Apache mod_fcgid, it
//! does not currently support "multiplexing", where 
//! multiple concurrent requests come into the same process.
//! Apache fcgid uses multiple processes for that. Safer.
//!
//  Animats
//  August, 2025
//
use anyhow::{Result, Error, anyhow};
use std::io;
use std::collections::{HashMap};
use std::io::{Read, Write, BufRead, Stdin, Stdout};
use std::io::{BufReader};
use num_derive::{FromPrimitive, ToPrimitive}; // Derive the FromPrimitive trait
use num_traits::{FromPrimitive, ToPrimitive};

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
/*

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
#[derive(Debug, FromPrimitive, ToPrimitive, Clone, PartialEq)]
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
#[derive(Debug, Clone)]
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
    /// Length of header
    pub const FCGI_HEADER_LENGTH: usize = 8;

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
    
    /// Serialize
    fn to_bytes(&self) -> [u8;8] {
        let id_bytes = self.id.to_be_bytes();
        let content_length_bytes = self.content_length.to_be_bytes();
        [
            self.version,   //  0
            self.rec_type.to_u8().unwrap(), // 1
            id_bytes[0],
            id_bytes[1],
            content_length_bytes[0],
            content_length_bytes[1],
            self.padding_length,// 7 provided but ignored
            0                  // 8 reserved
        ]
    }
}

/// FcgiRecord -- one header and its data.
///
/// Input is a stream of these.
#[derive(Debug)]
pub struct FcgiRecord {
    /// The header
    header: FcgiHeader,
    /// The content
    content: Option<Vec<u8>>
}

impl FcgiRecord {
    /// Read one record from stream.
    /// If Option<Request> is none, EOF has been reached.
    pub fn new_from_stream(instream: &mut impl BufRead) -> Result<Option<Self>, Error> {
        // Read header
        let mut header_bytes: [u8;FcgiHeader::FCGI_HEADER_LENGTH] = Default::default();
        let cnt = instream.read(&mut header_bytes)?;
        if cnt == 0 {
            return Ok(None) // normal EOF return
        }
        if cnt != FcgiHeader::FCGI_HEADER_LENGTH {
            return Err(anyhow!("FCGI header too short: {} bytes", cnt))
        }
        let header = FcgiHeader::new_from_bytes(&header_bytes)?;
        println!("Header: {:?}", header);   // ***TEMP***
        // Read content
        //////let mut content_bytes: [u8;header.content_length] = Default::default();
        let mut content_bytes = vec![0;header.content_length as usize];
        if header.content_length > 0 {
            let cnt = instream.read(&mut content_bytes)?;
            if cnt != content_bytes.len() {
                return Err(anyhow!("FCGI content too short: {} bytes", cnt))
            }
            if header.padding_length > 0 {
                let mut padding_bytes = vec![0;header.padding_length as usize];
                instream.read(&mut padding_bytes)?;
                if cnt != padding_bytes.len() {
                    return Err(anyhow!("FCGI padding too short: {} bytes", cnt))
                }
            }
        }
        Ok(Some(Self {
            header,
            content: Some(content_bytes.to_vec()),
        }))
    }
    
    /// Take content for use elsewhere
    pub fn take_content(&mut self) -> Option<Vec<u8>> {
        self.content.take()
    }   
}

/// Request to server.
#[derive (Debug)]
pub struct Request {
}

impl Request {
    /// New - reads a request from standard input.
    /// Can fail
    pub fn oldnew(instream: &mut impl BufRead) -> Result<Request, Error> {
        let mut header_bytes: [u8;FcgiHeader::FCGI_HEADER_LENGTH] = Default::default();
        instream.read(&mut header_bytes)?;
        let header = FcgiHeader::new_from_bytes(&header_bytes)?;
        println!("Header: {:?}", header);
        Ok(Request {

        })
    }
    
    //  Usual new
    pub fn new() -> Request {
        Self {
        }
    }
    
    /// True if ready to execute request.
    pub fn add_record(&mut self, rec: FcgiRecord) -> Result<bool, Error> {
        Ok(false)   // ***TEMP***
    }
}

/// Not the main program, but the main loop.
pub fn run(instream: &mut impl BufRead, out: &dyn Write, handler: fn(out: &dyn Write, request: &Request, env: &HashMap<String, String>) -> Result<i32>) -> Result<i32> {
    let env = std::env::vars().map(|(k,v)| (k,v)).collect();
    loop {
        let mut request = Request::new();
        if let Some(rec) = FcgiRecord::new_from_stream(instream)? {     
            if !request.add_record(rec)? {
                continue
            }
            // We have enough records to handle the request.
            handler(out, &request, &env)?;
        } else {
            return Ok(0);                  // normal EOF
        }
    }
}

#[test]
fn basic_io() {
    fn do_req<W: Write>(out: &dyn Write, request: &Request, env: &HashMap<String, String>) -> Result<i32> {
        Ok(200)   
    }
    let test_header = FcgiHeader { version: 1, rec_type: FcgiRecType::BeginRequest, id: 101, content_length: 16, padding_length: 0, reserved: 0 };
    let test_header_bytes = test_header.to_bytes();
    let mut test_data = test_header_bytes.to_vec();
    let test_content: Vec<u8> = "ABCDEFGHIJKLMNOP".as_bytes().to_vec();
    assert_eq!(test_content.len(), test_header.content_length as usize);
    test_data.extend(test_content);
    println!("Test data: {:?}", test_data);
    let cursor = std::io::Cursor::new(test_data);
    let mut instream = BufReader::new(cursor);
    let out = io::stdout();
    let final_result = run(&mut instream, &out, do_req::<&Stdout>);
    println!("Final result: {:?}", final_result);
}


//////fn handler(request: Request, env: HashMap<String, String>) ->Result<i32> 

//////fn handler(io: &mut dyn IO, env: HashMap<String, String>) -> Result<i32> {
