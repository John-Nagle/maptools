//! credentials -- manage database credentials and such
//!
//! Minimal low-security solution. Credentials are plain text
//! but not in a diirectory visible to the web server.

use std::path::{PathBuf};
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
    pub fn new(filename: &str) -> Result<Self, Error> {
        let path = Self::find_credentials(filename)?;
        let creds = match Envie::load_with_path(path.to_str().ok_or_else(|| anyhow!("Credentials filename {:?} has illegal UTF-8 characters.", path))?) {
            Ok(creds) => creds,
            Err(s) => { return Err(anyhow!("Error loading credentials: {}", s)); }
        };
        Ok(Self {
            creds
        })
    }
    //  Get value 	for key.
    pub fn get(&self, key: &str) -> Option<String> {
        self.creds.get(key)
    }
}

#[test]
fn test_credentials() {
    //  Test finding of file
    println!("Working directory: {:?}", std::env::current_dir());
    let cred_dir = Credentials::find_credentials(".bashrc").expect("Unable to find .bashrc");
    println!("Found {:?}", cred_dir);
    //  Test simple credentials file
    let creds = Credentials::new("test_credentials.txt").expect("Problem opening credentials file");
    assert_eq!("foo", creds.get("DEMO1").expect("Did not find key DEMO1").as_str());
    assert_eq!(Some("bar".to_string()), creds.get("DEMO2"));
}
