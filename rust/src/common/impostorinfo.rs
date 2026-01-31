//! Upload Second Life / Open Simulator terrain to server
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
//!
//! Once impostors have been created and uploaded to SL/OS, an LSL script tells this
//! server about them. Messages are JSON, and look roughly like this:
//!
//! {"comment":"Generated from sculpt texture UUIDS fetched from inventory","version":"1.0","name":"Blake Sea - Kraken",
//!  "region_loc":[290304,268288],"region_size":[256,256],"grid":"agni","elevation_offset":0.000000,
//!    "scale":[256,256,25.690001],"water_height":20.000000,"sculpt_uuid":"64604b5c-461e-dd72-52a9-3d464abf78aa","impostor_lod":0},
//! 
//! This is very close to the JSON sent to the viewer.
//
use anyhow::{anyhow, Error};
use uuid::Uuid;
use serde;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap};
/// The data stored in the database for a region impostor.
///
/// This is very similar to the version inside Sharpview at
/// https://github.com/John-Nagle/SL-test-viewer/blob/main/libclient/src/impostor/regionimpostor.rs#L480
/// 
/// Region impostors are mesh or sculpts.
/// Mesh objects are -0.5 .. 0.5 in each axis, as is
/// normal for SL/OS. The mesh will be scaled to fit the region box.
/// Mesh impostor objects are aligned with the world coordinate system.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RegionImpostorData {
    /// Where it is in the world.
    pub region_loc: [u32;2],
    /// Size of the region (always 256,256 for SL)
    pub region_size: [u32;2],
    /// Scale of the impostor mesh object (because mesh objects are all scaled -0.5 .. 0.5
    pub scale: [f32;3],
    /// Impostor level of detail. 0=1 region, 1=4 regions, etc.
    pub impostor_lod: RegionImpostorLod,
    /// Viz group ID. You can only see objects with the same viz group ID as your own.
    /// This indicates reachability without a teleport.
    /// Viz groups are generally in order of decreasing
    pub viz_group: u32,
    /// The object geometry, as a sculpt image.
    pub sculpt_uuid: Option<Uuid>,    
    /// Hash of the info used to build the sculpt, for change detection.
    pub sculpt_hash: Option<String>,
    /// The object geometry, as a mesh object. If both are present, use mesh.
    pub mesh_uuid: Option<Uuid>,
    /// Hash of the info used to buld the mesh, for change detection.
    pub mesh_hash: Option<String>,
    /// Base of object is at this level. 
    /// Should be zero unless this is a mountain range.
    pub elevation_offset: f32,
    /// Water height. Water is optional.
    pub water_height: Option<f32>,
    /// Name - name of region, if available. Mostly for debug.
    pub name: Option<String>,
    /// Grid -- name of associated grid
    pub grid: String,
    /// Faces (as JSON)
    pub faces: Vec<RegionImpostorFaceData>,
}

pub type RegionImpostorLod = u8;

impl RegionImpostorData {
}
/// Data for each face.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RegionImpostorFaceData {
    /// Base texture for old non-material objects.
    pub base_texture_uuid: Uuid,
    /// Emissive texture, to show what's lit at night.
    /// For now, this is future expansion.
    pub emissive_texture_uuid: Option<Uuid>,
    /// Hash to avoid unnecessary asset uploads.
    pub base_texture_hash: String,
    /// Hash to avoid unnecessary asset uploads
    pub emissive_texture_hash: Option<String>,
}

impl RegionImpostorFaceData {
    /// Make JSON from array of tuples.
    /// Tuples are in order but may be sparse.
    /// JSON must be an array in texture index order but rows can be empty.
    /// This requires excessive wrangling.
    pub fn json_from_tuples(tuples: &Vec<(usize, String, String, String)>) -> Result<serde_json::Value, Error> {
        const MAX_TEXTURES: usize = 8;
        let mut base_textures: [Option<String>;MAX_TEXTURES] = Default::default();
        let mut emissive_textures: [Option<String>;MAX_TEXTURES] = Default::default();
        for (texture_index, texture_uuid, texture_hash, asset_type) in tuples {
            let arr = match asset_type.as_str() {
                "BaseTexture" => &mut base_textures,
                "EmissiveTexture" => &mut emissive_textures,
                _ => { return Err(anyhow!("Invalid asset type for face data: {}", asset_type)); }
            };
            if *texture_index >= MAX_TEXTURES {
                return Err(anyhow!("Out of range texture index {} asset type for face data: {}", texture_index, asset_type));
            }
            if arr[*texture_index].is_some() {
                return Err(anyhow!("Duplicate texture index {} asset type for face data: {}", texture_index, asset_type)); 
            }
            arr[*texture_index] = Some(texture_uuid.to_string());
        }
        //  Now we have arrays of tuples. Convert to a vec of structs, stopping at the last non-empty.
        let mut face_data = Vec::new();
        let cnt = MAX_TEXTURES; // Cover all the slots
        for n in 0..cnt {
            //  Stop at first empty slot. 
            //  Sparse texture usage not supported.
            if base_textures[n].is_none() {
                break;
            }
            let mut vals = serde_json::Map::new();
            let mut inserter = |k: &str, v: &str| { vals.insert(k.to_string(), serde_json::Value::String(v.to_string())) };
            //  Not putting the hashes in the JSON because the viewer does not need them.
            if let Some(v) = &emissive_textures[n] {
                inserter("base_texture_uuid", v);
            }
            if let Some(v) = &emissive_textures[n] {
                inserter("emissive_texture_uuid", v);
            }
            face_data.push(serde_json::Value::Object(vals));
        }
        let face_json = serde_json::Value::Array(face_data);
        log::debug!("Face JSON: {:?}", face_json);
        Ok(face_json)
    }    
}

/// What's returned to a caller via a REST request
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RegionImpostorReply {
    /// Data version
    pub version: u32,
    /// The impostors
    pub impostors: Vec<RegionImpostorData>,
    /// Errors, if any
    pub errors: Vec<String>,
}

impl RegionImpostorReply {
    /// Version of this interface
    pub const REGION_IMPOSTOR_INFO_VERSION: u32 = 1;
}
