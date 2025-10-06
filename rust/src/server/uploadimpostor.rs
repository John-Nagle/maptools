//! Upload Second Life / Open Simulator terrain to server
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
//!
//! Once impostors have been created and uploaded to SL/OS, an LSL script tells this
//! server about them. Messages are JSON, and look roughly like this:
//!
//! {"comment":"Generated from sculpt texture UUIDS fetched from inventory","version":"1.0","name":"Blake Sea - Kraken",
//!  "region_loc":[290304,268288],"region_size":[256,256],"grid":"agni","elevation_offset":0.000000,
//!    "scale":[256,256,25.690001],"water_height":20.000000,"sculpt_uuid":"64604b5c-461e-dd72-52a9-3d464abf78aa","impostor_lod":0},
//! 
//! This is very close to the JSON sent to the viewer.

use anyhow::{Error, anyhow};
/*
use log::LevelFilter;
use common::Credentials;
use common::init_fcgi;
use common::{Handler, Request, Response};
use common::{UploadedRegionInfo};
use common::u8_to_elev;
use mysql::prelude::{Queryable};
use mysql::{Pool};
use mysql::{PooledConn, params};
use std::collections::HashMap;
use std::io::Write;
mod auth;
use auth::{Authorizer, AuthorizeType};
*/

/*
CREATE TABLE IF NOT EXISTS region_impostors (
    grid VARCHAR(40) NOT NULL,
    name VARCHAR(100) NOT NULL,
    region_loc_x INT NOT NULL,
    region_loc_y INT NOT NULL,
    region_size_x INT NOT NULL,
    region_size_y INT NOT NULL,
    scale_x INT NOT NULL,
    scale_y INT NOT NULL,
    scale_z FLOAT NOT NULL,
    elevation_offset FLOAT NOT NULL,
    impostor_lod TINYINT NOT NULL,
    viz_group INT NOT NULL,
    mesh_uuid CHAR(40) DEFAULT NULL,
    sculpt_uuid CHAR(36) DEFAULT NULL,
    water_height FLOAT NOT NULL,
    creator VARCHAR(63) NOT NULL,
    creation_time TIMESTAMP NOT NULL,
    faces_json JSON NOT NULL,
    UNIQUE INDEX (grid, region_loc_x, region_loc_y, impostor_lod),
    INDEX(grid, viz_group),
    INDEX(name)
)

-- What to do about faces? Another table? Or 8 slots here?
-- "faces": [ 
-- { "base_texture_uuid": "acde070d-8c4c-4f0d-9d8a-162843c10333",
-- "emissive_texture_uuid": null
 }
 
 Use struct from https://github.com/John-Nagle/SL-test-viewer/blob/main/libclient/src/impostor/regionimpostor.rs#L480
 */
 


