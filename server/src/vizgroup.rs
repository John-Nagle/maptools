//! vizgroup.rs - compute the visibility groups of regions.
//!
//! The visibiilty groups indicate which regions can be seen
//! from which other regions. If there is a path of connected
//! regions from one region to another, the regions can see
//! each other. 
//!
//! It's a transitive closure on "adjacent".
//! 
//! Corners are adjacent on Open Simulator but not Second Life.
//!
//! Animats
//! September, 2025
//! License: LGPL.
//!
use mysql::{OptsBuilder, Opts, Conn, Pool};
use serde::{Deserialize};
use mysql::{PooledConn, params};
use mysql::prelude::{Queryable, AsStatement};
use anyhow::{Error, anyhow};

/// Vizgroups - find all the visibility groups
pub struct Vizgroups {
}

impl Vizgroups {
    /// Usual new
    pub fn new() -> Self {
        Self {
        }
    }
    
    /// Build from database
    pub fn build(&mut self, conn: PooledConn) -> Result<(), Error> {
        println!("Build start");    // ***TEMP***
        Ok(())
    }
}
