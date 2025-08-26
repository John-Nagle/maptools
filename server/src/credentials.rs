//! credentials -- manage database credentials and such


use std::path::{Path, PathBuf};
use std::collections::HashMap;

/// Key/value store for credentials
pub struct Credentials {
    /// The credentials
    creds: HashMap<String, String>,
}

impl Credentials {

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
}
