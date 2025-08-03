//! FCGI echo server
//! 
//! Follows example for "vintage"
use vintage::{Response, ServerConfig};

fn main() {

    let config = ServerConfig::new()
        .on_get(["/about"], |_req, _params| {
            Response::html("<h1>Hello World</h1>")
        });

    let handle = vintage::start(config, "localhost:0").unwrap();

// This would block the current thread until the server thread exits
// handle.join()

// Graceful shutdown
    handle.stop();
}
