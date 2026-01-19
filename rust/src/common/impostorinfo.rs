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
use uuid::Uuid;
use serde;
use serde::{Deserialize, Serialize};
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
    /// Estate ID. You can only see objects with the same estate ID as your own.
    /// This indicates reachability without a teleport.
    /// By convention, mainland is estate id 1.
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
    /// Name - name of region, or 0,0 corner region for lower LODs
    pub name: String,
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
}

impl RegionImpostorFaceData {

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
