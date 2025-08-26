//! credentials -- manage database credentials and such


use std::path::{Path, PathBuf};
use envie::{Envie};
use anyhow::{Error, anyhow};

/// Key/value store for credentials
pub struct Credentials {
    /// The credentials
    creds: Envie,
}

impl Credentials {

    /// Find credentials file.
    /// Look in parent directories.
    fn find_credentials(filename: &str) -> Result<PathBuf, Error> {
        let mut wd = std::env::current_dir()?;   // start at current directory
        // Go up the tree. Prevent runaway.
        for _ in 0..100 {
            //  Valid directory?
            if !wd.exists() {
                return Err(anyhow!("Tried all parent directories without finding credentials."));
            }
            //  Is it in this directory
            let mut cred_path = wd.clone();
            cred_path.push(filename);
            if cred_path.exists() {
                return Ok(cred_path)
            }
            //  No, try parent directory.
            wd = wd.parent().ok_or_else(|| anyhow!("Could not find credentials file {:?} in directory tree.", filename))?.to_path_buf(); 
        }
        Err(anyhow!("Link loop in directory tree above {:?}", wd))
    }

    /// Usual new.
    /// Initializes the credentials
    pub fn new() -> Option<Self> {
        todo!();
    }
    //  Get value for key.
    pub fn get_value(key: &str) -> Option<String> {
        todo!();
    }
}

#[test]
fn test_credentials() {
    let cred_dir = Credentials::find_credentials(".bashrc").expect("Unable to find .bashrc");
    println!("Found {:?}", cred_dir);
}
