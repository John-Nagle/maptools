//! httpretry.rs -- HTTP with retry
//!
//! Useful when talking to flaky servers
//!
//! Animats
//! April, 2026
use ureq::{Agent, http::Response, Body, Error};
/// Retry this many times.
const RETRY_MAX: usize = 5;
/// HTTP Get with retry
pub fn get_with_retry(agent: &mut Agent, url: &str) -> Result<Response<Body>, Error> {
    let mut retries: usize = 0;
    loop {
        match agent.get(url).call() {
            Ok(response) => {
                //  Success
                return Ok(response)
            }
            Err(ureq::Error::StatusCode(code)) => {
                // the server returned an unexpected status
                log::debug!("HTTP fail, code {}, reading {}", code, url);
                match code {
                    //  No find is not an error, just a hole in the map.
                    403 => return Err(ureq::Error::StatusCode(code)),
                    404 => return Err(ureq::Error::StatusCode(code)),
                    _ => {
                        if retries > RETRY_MAX {
                            log::warn!("HTTP error status {} reading {}", code, url);
                            return Err(ureq::Error::StatusCode(code));
                        }
                        log::warn!("HTTP error status {} reading {}, retrying.", code, url);
                    } 
                }
            }
            Err(e) => {
                //  Network error
                if retries > RETRY_MAX {
                    log::warn!("HTTP network error {:?} reading {}", e, url);
                    return Err(e);
                }
                log::warn!("HTTP network error {:?} reading {}, retrying.", e, url);              
            }
        }
        retries += 1;
        log::warn!("Retrying HTTP GET for {}, retry #{}", url, retries);
    }
}
