use std::collections::HashMap;
use outer_cgi::IO;

fn handler(io: &mut dyn IO, env: HashMap<String, String>) -> anyhow::Result<i32> {
    let mut all_data = Vec::new();
    let sink = io.read_to_end(&mut all_data)?;
    io.write_all(format!(r#"Content-type: text/plain; charset=utf-8

Hello World! Your request method was "{}"!
"#, env.get("REQUEST_METHOD").unwrap()).as_bytes())?;
    Ok(0)
}

pub fn main() {
    outer_cgi::main(|_|{}, handler)
}
