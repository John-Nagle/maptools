//! Mini FCGI framework.
//!
//! Used by FCGI applications (responders)
//! called from Apache-type web servers.
//!
//! Reads and writes to anything that talks Rust read/write.
//!
//! Normal usage:
//!
//!    pub fn main() {
//!        minifcgi::run(|_|{}, handler)
//!    }
//!
//!
//! Since this code is intended to support only Apache mod_fcgid, it
//! does not currently support "multiplexing", where
//! multiple concurrent requests come into the same process.
//! Apache fcgid uses multiple processes for that. Safer.
//!
//  Animats
//  August, 2025
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
//
// See the [Common Gateway Interface][1] specification for more information.
//
// [1]: https://tools.ietf.org/html/rfc3875
//
/// and the FastCGI specification:
//
// https://www.mit.edu/~yandros/doc/specs/fcgi-spec.html
//
// An implemention in Go, for comparison: see https://cs.opensource.google/go/go/+/master:src/net/http/fcgi/fcgi.go

//
use anyhow::{Error, Result, anyhow};
use num_derive::{FromPrimitive, ToPrimitive}; // Derive the FromPrimitive trait
use num_traits::{FromPrimitive, ToPrimitive};
use std::collections::HashMap;
use std::io::{BufRead, Write};
/// Trait for callback
pub trait Handler {
    /// caller must provide handler fn
    fn handler(
        &mut self,
        out: &mut dyn Write,
        request: &Request,
        env: &HashMap<String, String>,
    ) -> Result<(), Error>;
}

/// Type of transaction. Only Responder is implemented.
#[derive(Debug, FromPrimitive, ToPrimitive, Clone, PartialEq)]
enum _FcgiRole {
    /// Respond and execute commands
    Responder = 1,
    /// Authorization (unimplemented)
    Authorizer = 2,
    /// Filter (unimplemented)
    Filter = 3,
}

/// Response status
#[derive(Debug, FromPrimitive, ToPrimitive, Clone, PartialEq)]
enum FcgiStatus {
    /// Normal
    RequestComplete = 0,
    /// Saw more than one ID
    CantMultiplex = 1,
    /// Overloaded, although that can't really happen here.
    Overloaded = 2,
    /// Unknown role, something other than Responder.
    UnknownRole = 3,
}

/// Type of FCGI record. Almost always BeginRequest, Params, or Stdin.
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
    content_length: u16,
    /// Length of padding, in bytes
    padding_length: u8,
}

impl FcgiHeader {
    /// Length of header
    pub const FCGI_HEADER_LENGTH: usize = 8;

    /// Deserialize 8 bytes to an FCGI header.
    fn new_from_bytes(b: &[u8; 8]) -> Result<FcgiHeader, Error> {
        let content_length = u16::from_be_bytes(<[u8; 2]>::try_from(&b[4..6]).unwrap());
        let header = FcgiHeader {
            version: b[0],
            rec_type: FcgiRecType::from_u8(b[1])
                .ok_or_else(|| anyhow!("Invalid FCGI record type: {}", b[1]))?,
            id: u16::from_be_bytes(<[u8; 2]>::try_from(&b[2..4]).unwrap()),
            content_length,
            padding_length: b[7],
        };
        if header.padding_length != Self::calc_padding_length(content_length) {
            log::error!(
                "Received padding length {}, calculated padding length {}",
                header.padding_length,
                Self::calc_padding_length(content_length)
            );
        }
        log::info!("FCGI header: {:?}", header);
        Ok(header)
    }

    /// Serialize an FCGI header to 8 bytes.
    fn to_bytes(&self) -> [u8; 8] {
        let id_bytes = self.id.to_be_bytes();
        let content_length_bytes = self.content_length.to_be_bytes();
        [
            self.version,                   //  0
            self.rec_type.to_u8().unwrap(), // 1
            id_bytes[0],
            id_bytes[1],
            content_length_bytes[0],
            content_length_bytes[1],
            self.padding_length, // padding is optional, per spec
            0,                   // 8 reserved
        ]
    }

    /// padding needed to round up to next multiple of 8
    fn calc_padding_length(content_length: u16) -> u8 {
        (8 - u8::try_from(content_length & 0x7).unwrap()) & 0x7
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
    content: Option<Vec<u8>>,
}

impl FcgiRecord {
    /// Read one record from stream.
    /// If Option<Request> is none, EOF has been reached.
    pub fn new_from_stream(instream: &mut impl BufRead) -> Result<Option<Self>, Error> {
        // Read header
        let mut header_bytes: [u8; FcgiHeader::FCGI_HEADER_LENGTH] = Default::default();
        log::debug!("About to read {} header bytes.", header_bytes.len());
        match instream.read_exact(&mut header_bytes) {
            Ok(_) => {} // read expected data
            Err(e) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    return Ok(None); // Normal EOF exit - end of file at correct point
                }
                return Err(e.into());
            }
        }
        let header = FcgiHeader::new_from_bytes(&header_bytes)?;
        log::debug!("header: {:?}", header);
        // Read content
        let mut content_bytes = vec![0; header.content_length as usize];
        if header.content_length > 0 {
            log::debug!("About to read {} content bytes", content_bytes.len());
            instream.read_exact(&mut content_bytes)?;
            log::debug!(
                "Content: {:?}",
                String::from_utf8_lossy(&content_bytes[0..content_bytes.len().min(200)].to_vec())
            );
            let padding_length = header.padding_length;
            if padding_length > 0 {
                let mut padding_bytes = vec![0; padding_length as usize];
                log::debug!("About to read {} padding bytes", padding_bytes.len());
                instream.read_exact(&mut padding_bytes)?;
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
#[derive(Debug)]
pub struct Request {
    /// The header
    id: Option<u16>,
    /// Parameter bytes. Need special decoding
    param_bytes: Vec<u8>,
    /// Params, as a key-value store
    pub params: Option<HashMap<String, String>>,
    /// Standard input - the actual content, if any. Usually from a POST request.
    pub standard_input: Vec<u8>,
}

impl Request {
    ///  Usual new
    pub fn new() -> Request {
        Self {
            id: None,
            param_bytes: Vec::new(),
            standard_input: Vec::new(),
            params: None,
        }
    }

    /// True if ready to execute request.
    pub fn add_record(&mut self, mut rec: FcgiRecord) -> Result<bool, Error> {
        //  Check that we're not in multiplex mode
        if self.id.is_some() {
            if self.id.unwrap() != rec.header.id {
                return Err(anyhow!(
                    "FCGI record IDs differ. Multiplex mode not supported."
                ));
            }
        } else {
            self.id = Some(rec.header.id)
        }
        // Fan out on type.
        match rec.header.rec_type {
            FcgiRecType::BeginRequest => {
                //  Content should be {FCGI_RESPONDER, 0}
            }

            FcgiRecType::Params => {
                // More param bytes
                let content = rec
                    .content
                    .take()
                    .ok_or_else(|| anyhow!("No params content. Should not happen."))?;
                self.param_bytes.extend_from_slice(&content);
            }

            FcgiRecType::Stdin => {
                //  A zero-length block means we have a complete request .
                if rec.header.content_length == 0 {
                    self.params = Some(Self::build_params(&self.param_bytes)?);
                    log::debug!("Params: {:?}", self.params);
                    //  Request now gets processed.
                    return Ok(true);
                }
                let content = rec
                    .content
                    .take()
                    .ok_or_else(|| anyhow!("No content. Should not happen."))?;
                //  Optimization to prevent unnecessary copy of content, which can be very large.
                if self.standard_input.is_empty() {
                    self.standard_input = content;
                } else {
                    self.standard_input.extend_from_slice(&content);
                }
            }
            _ => {
                return Err(anyhow!(
                    "FCGI responder record type {:?} unknown or unimplemented.",
                    rec.header.rec_type
                ));
            }
        }
        //  Not done with request, keep reading records
        Ok(false)
    }

    /// Fetch one encoded value.
    /// 0..127 is one byte.
    /// If the first byte is larger than 127, fetch 3 more bytes and convert to a usize
    fn fetch_field_length<'a>(
        mut pos: impl Iterator<Item = &'a u8>,
    ) -> Result<Option<usize>, Error> {
        if let Some(b0) = pos.next() {
            if *b0 > 127 {
                //  Fetch 3 more bytes
                let b1 = pos.next().ok_or_else(|| {
                    anyhow!("FCGI responder: EOF reading multi-byte param length")
                })?;
                let b2 = pos.next().ok_or_else(|| {
                    anyhow!("FCGI responder: EOF reading multi-byte param length")
                })?;
                let b3 = pos.next().ok_or_else(|| {
                    anyhow!("FCGI responder: EOF reading multi-byte param length")
                })?;
                //  Compute length per spec
                let v = (((*b0 & 0x7f) as usize) << 24)
                    + ((*b1 as usize) << 16)
                    + ((*b2 as usize) << 8)
                    + *b3 as usize;
                log::debug!(
                    "Param length, multibyte: {:02x} {:02x} {:02x} {:02x} -> {:08x}",
                    b3,
                    b2,
                    b1,
                    b0,
                    v
                );
                Ok(Some(v))
            } else {
                Ok(Some(*b0 as usize))
            }
        } else {
            Ok(None) // EOF
        }
    }

    /// Fetch FCGI param field of requested length. Read N bytes, convert to UTF-8. Error if bad UTF-8.
    fn fetch_field<'a>(cnt: usize, mut pos: impl Iterator<Item = &'a u8>) -> Result<String, Error> {
        let mut b = Vec::with_capacity(cnt);
        for _ in 0..cnt {
            let ch = pos
                .next()
                .ok_or_else(|| anyhow!("FCGI responder: EOF reading param field"))?;
            b.push(*ch);
        }
        Ok(String::from_utf8(b)?.to_string())
    }

    /// "FastCGI transmits a name-value pair as the length of the name, followed by the length of the value, followed by the name, followed by the value.
    /// Lengths of 127 bytes and less can be encoded in one byte, while longer lengths are always encoded in four bytes" - FCGI spec
    fn fetch_name_value_pair<'a>(
        mut pos: impl Iterator<Item = &'a u8>,
    ) -> Result<Option<(String, String)>, Error> {
        if let Some(kcnt) = Self::fetch_field_length(&mut pos)? {
            if let Some(vcnt) = Self::fetch_field_length(&mut pos)? {
                Ok(Some((
                    Self::fetch_field(kcnt, &mut pos)?,
                    Self::fetch_field(vcnt, &mut pos)?,
                )))
            } else {
                Err(anyhow!(
                    "FCGI responder: EOF reading length of param value field"
                ))
            }
        } else {
            Ok(None) // EOF
        }
    }

    /// Build key-value list from special format.
    pub fn build_params(b: &[u8]) -> Result<HashMap<String, String>, Error> {
        log::debug!(
            "Param bytes: {:?}",
            String::from_utf8_lossy(&b[0..b.len().min(2000)].to_vec())
        );
        let mut m = HashMap::new();
        let mut pos = b.iter();
        while let Some((k, v)) = Self::fetch_name_value_pair(&mut pos)? {
            log::debug!("Param: \"{}\" = \"{}\"", k, v);
            m.insert(k, v);
        }
        Ok(m)
    }
}

/// Response -- sends back a response to a request.
pub struct Response {}

impl Response {
    /// Padding responses is optional, per spec.
    /// mod_fcgid isn't padding its messages to us.
    const PAD_RESPONSES: bool = false;

    /// Write one response record.
    fn write_response_record(
        out: &mut dyn Write,
        request: &Request,
        rec_type: FcgiRecType,
        b: &[u8],
    ) -> Result<(), Error> {
        assert!(b.len() < u16::MAX.into());
        let padding_length = if Self::PAD_RESPONSES {
            //  Rounds up to 8 bytes
            FcgiHeader::calc_padding_length(b.len() as u16)
        } else {
            0
        };
        let header = FcgiHeader {
            version: 1,
            rec_type,
            id: request.id.expect("No request ID"),
            content_length: b.len() as u16,
            padding_length,
        };
        log::debug!(
            "Writing response record: {:?} Data: {:?}",
            header,
            String::from_utf8_lossy(&b[0..b.len().min(200)].to_vec())
        );
        //  Write header
        out.write(&header.to_bytes())?;
        //  Write data
        if b.len() > 0 {
            out.write(b)?;
        }
        //  Write padding
        if header.padding_length > 0 {
            let padding_bytes = vec![0; header.padding_length as usize];
            out.write(&padding_bytes)?;
        }
        Ok(())
    }

    /// Write entire response.
    ///    {FCGI_STDOUT,      1, "Content-type: text/html\r\n\r\n<html>\n<head> ... "}
    ///    {FCGI_STDOUT,      1, ""}
    ///    {FCGI_END_REQUEST, 1, {0, FCGI_REQUEST_COMPLETE}}
    pub fn write_response(
        out: &mut dyn Write,
        request: &Request,
        header_fields: &[String],
        b: &[u8],
    ) -> Result<(), Error> {
        //  Send header fields
        let header_fields_group = header_fields.join("\r\n") + "\n\n";
        log::info!("Response header: {}", header_fields_group);
        Self::write_response_record(
            out,
            request,
            FcgiRecType::Stdout,
            &header_fields_group.as_bytes(),
        )?;
        //  End of HTTP header record.
        Self::write_response_record(out, request, FcgiRecType::Stdout, "".as_bytes())?;
        //  Only send this much data at once to avoid clogging pipe.
        //  The connection to the parent process is two pipes in opposite directions and deadlock is possible.
        const CHUNK_SIZE: usize = 2048;
        for i in (0..b.len()).step_by(CHUNK_SIZE) {
            Self::write_response_record(
                out,
                request,
                FcgiRecType::Stdout,
                &b[i..(i + CHUNK_SIZE).min(b.len())],
            )?;
        }
        //  End of data record.
        Self::write_response_record(out, request, FcgiRecType::Stdout, &[])?;
        // End of transaction record.
        Self::write_response_record(
            out,
            request,
            FcgiRecType::EndRequest,
            &[0, FcgiStatus::RequestComplete.to_u8().unwrap()],
        )?;
        out.flush()?;
        Ok(())
    }

    /// Build the most common response headers.
    pub fn http_response(content_type: &str, status: usize, msg: &str) -> Vec<String> {
        vec![
            format!("Status: {} {}", status, msg),
            format!("Content-Type: {}; charset=utf-8", content_type),
        ]
    }
}

/// Read and run one transaction.
/// Errors here result in a 500 error with a message.
fn run_one<T: Handler>(
    instream: &mut impl BufRead,
    out: &mut dyn Write,
    request: &mut Request,
    handler: &mut T,
    env: &HashMap<String, String>,
) -> Result<bool, Error> {
    loop {
        if let Some(rec) = FcgiRecord::new_from_stream(instream)? {
            if !request.add_record(rec)? {
                continue;
            }
            // We have enough records to handle the request.
            handler.handler(out, &request, &env)?;
            break;
        } else {
            return Ok(true); // normal EOF
        }
    }
    Ok(false)
}

/// Not the main program, but the main loop.
pub fn run<T: Handler>(
    instream: &mut impl BufRead,
    out: &mut dyn Write,
    handler: &mut T,
) -> Result<(), Error> {
    let env = std::env::vars().map(|(k, v)| (k, v)).collect();
    let mut request = Request::new();
    loop {
        match run_one(instream, out, &mut request, handler, &env) {
            Ok(done) => {
                if done {
                    //  Normal end of this task.
                    break;
                }
            }
            Err(e) => {
                //  Error occured. Try to get it back to the caller.
                let msg = format!("FCGI responder error: {:?}", e);
                log::error!("{}", msg);
                if request.id.is_some() {
                    //  We have enough info to reply with an error
                    let error_response = Response::http_response("text", 500, msg.as_str());
                    Response::write_response(out, &request, error_response.as_slice(), &[])?;
                    break;
                } else {
                    //  Failed so early we can't reply with an error.
                    panic!("FCGI responder failed before first record parsed: {}", msg);
                }
            }
        }
    }
    Ok(())
}

#[test]
fn basic_io() {
    use std::io::{BufReader, Write};
    //  Our data
    struct TestHandler {
        cnt: usize,
    }
    impl TestHandler {
        pub fn new() -> Self {
            Self { cnt: 0 }
        }
    }
    //  Our "handler"
    impl Handler for TestHandler {
        fn handler(
            &mut self,
            out: &mut dyn Write,
            request: &Request,
            env: &HashMap<String, String>,
        ) -> Result<(), Error> {
            // Dummy up a response
            self.cnt += 1;
            let http_response = Response::http_response("text/plain", 200, "OK");
            let b = format!("Env: {:?}\nParams: {:?}", env, request.params).into_bytes();
            Response::write_response(out, request, http_response.as_slice(), &b)?;
            Ok(())
        }
    }
    //  BeginRequest
    let test_header0 = FcgiHeader {
        version: 1,
        rec_type: FcgiRecType::BeginRequest,
        id: 101,
        content_length: 16,
        padding_length: 0,
    };
    let test_header0_bytes = test_header0.to_bytes();
    let mut test_data = test_header0_bytes.to_vec();
    //  ***NOT A VALID BEGIN REQUEST***
    let test_content0: Vec<u8> = "ABCDEFGHIJKLMNOP".as_bytes().to_vec();
    assert_eq!(test_content0.len(), test_header0.content_length as usize);
    test_data.extend(test_content0);
    //  Params
    let test_header1 = FcgiHeader {
        version: 1,
        rec_type: FcgiRecType::Params,
        id: 101,
        content_length: 10,
        padding_length: 0,
    };
    let test_header1_bytes = test_header1.to_bytes();
    let test_content1: Vec<u8> = vec![
        3, 5, 'K' as u8, 'E' as u8, 'Y' as u8, 'V' as u8, 'A' as u8, 'L' as u8, 'U' as u8,
        'E' as u8,
    ];
    assert_eq!(test_content1.len(), test_header1.content_length as usize);
    test_data.extend(test_header1_bytes);
    test_data.extend(test_content1);
    //  Stdin - empty content is an EOF
    let test_header2 = FcgiHeader {
        version: 1,
        rec_type: FcgiRecType::Stdin,
        id: 101,
        content_length: 0,
        padding_length: 0,
    };
    test_data.extend(test_header2.to_bytes());
    println!("Test data: {:?}", test_data);
    let cursor = std::io::Cursor::new(test_data);
    let mut instream = BufReader::new(cursor);
    let mut out = std::io::stdout();
    let mut test_handler = TestHandler::new();
    run(&mut instream, &mut out, &mut test_handler).expect("Run failed");
}
