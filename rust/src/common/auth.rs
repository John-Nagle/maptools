//! auth.rs -- server side authorization manager
//!
//! Part of the Animats impostor system
//!
//! A Second Life/Open Simulator LSL script records terrain heights when visiting
//! regions. It calls this FCGI responder to upload that data to a server.
//! Later processing turns that into objects viewable in world via the
//! region impostor system.
//!
//! License: LGPL.
//! Animats
//! August, 2025.
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

/// Environment variables for obtaining owner info.
/// ***ADD VALUES FOR OPEN SIMULATOR***
const OWNER_NAME_PARAMS: [&str;1] = ["HTTP_X_SECONDLIFE_OWNER_NAME"];


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
    pub fn authorize(auth_type: AuthorizeType, env: &HashMap<String, String>, params: &HashMap<String, String>) -> Result<String, Error> {
        if let Some(owner_name) =  OWNER_NAME_PARAMS.iter().find_map(|&s| params.get(s)) {
            log::info!("Request is from an object owned by {}", owner_name);
            Ok(owner_name.trim().to_string())   
        } else {
            Err(anyhow!("This request is not from Second Life/Open Simulator"))
        }
    }
}
