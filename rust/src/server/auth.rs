//! auth.rs -- server side authorization manager
//!
//! Part of the Animats impostor system
//!
//! A Second Life/Open Simulator LSL script records terrain heights when visiting
//! regions. It calls this FCGI responder to upload that data to a server.
//! Later processing turns that into objects viewable in world via the
//! region impostor system.
//!
//!     License: LGPL.
//!     Animats
//!     August, 2025.
//
use anyhow::{Error, anyhow};
use std::collections::HashMap;
/*
use common::Credentials;
use common::init_fcgi;
use common::{Handler, Request, Response};
use common::{UploadedRegionInfo};
use common::u8_to_elev;
use mysql::prelude::{Queryable};
use mysql::{Pool};
use mysql::{PooledConn, params};

*/

pub enum AuthorizeType {
    /// Upload terrain. Can add and update terrain data.
    UploadTerrain,
    /// Upload impostors. Can add and upload impostor data.
    UploadImpostors,
}

pub struct Authorizer {
}

impl Authorizer {
    /// External caller requests permission to do something.
    pub fn authorize(auth_type: AuthorizeType, env: &HashMap<String, String>, params: &HashMap<String, String>) -> Result<(), Error> {
        Ok(())  // ***TEMP***
    }
}
