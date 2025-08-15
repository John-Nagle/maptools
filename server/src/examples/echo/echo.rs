//! FCGI echo server. 
//! For test use.
use std::io::Write;
use std::collections::HashMap;
use std::io::BufReader;
use minifcgi;
use minifcgi::{Request, Response};
use anyhow::{Error};

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

/// Handler. actually handles the FCGI request.
fn handler(out: &dyn Write, request: &Request, env: &HashMap<String, String>) -> Result<i32, Error> {
    //  ***MORE***
    Ok(0)
}

pub fn main() {
    let mut inio = std::io::stdin();
    let mut outio = std::io::stdout();
    let mut instream = BufReader::new(inio);
    minifcgi::run(&mut instream, &mut outio, handler);
}
