//! initialimpostors.rs -- generate the initial_impostor_regions table.
//!
//! The initial_impostor_regions table is the basis for the final impostor_regions
//! table. It has everything except the UUIDs of assets which still need to be created.
//! It's created here, and uploadterrain updates it with new assets.
//! When all UUIDs are non-null, the impostor_regions info is complete, and
//! this table is copied over to the impostor_regions table as an atomic operation.
//!
//! Part of the Animats impostor system
//!
//!
//!     License: LGPL.
//!     Animats
//!     February, 2026.
//
use anyhow::{Error, anyhow};
use mysql::{PooledConn};
//////use mysql::prelude::{Queryable};

/// The initial impostors.
pub struct InitialImpostors {
    /// SQL connection
    conn: PooledConn,
}

impl InitialImpostors {
    /// Usual new
    pub fn new(conn: PooledConn) -> Result<Self, Error> {

        //  ***CLEAR initial_region_impostors table***
        Ok(Self {
            conn
        })
    }
    
    /// Add one tile (sculpt or mesh) to the table. UUIDs may be null.
    pub fn add_tile(&mut self) -> Result<(), Error> {
        todo!();
    }
}
