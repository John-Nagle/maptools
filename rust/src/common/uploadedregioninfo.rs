//! Access to raw terrain table in SQL.
//!
//! License: LGPL.
//! Animats
//! August, 2025.
//
use anyhow::{Error, anyhow};
use serde::Deserialize;
///  Our data as uploaded from SL/OS in JSON format
// "{\"region\":\"Vallone\",\"scale\":1.092822,\"offset\":33.500740,\"waterlev\":20.000000,\"regioncoords\":[1807,1199],
//  \"elevs\":[\"E7CAACA3A5A8ACAEB0B2B5B9BDC0C4C5C5C3C0BDB9B6B3B2B2B3B4B7BBBFC3C7CBCED1D3D5D5D4CFC4B5A4"";
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UploadedRegionInfo {
    /// Grid name
    pub grid: String,
    /// Position of region in world, meters.
    pub region_coords: [u32; 2],
    /// Region size. 256 x 256 if ommitted.
    pub size: Option<[u32; 2]>,
    /// Region name
    pub name: String,
    /// Height data, a long set of hex data.  
    elevs: Vec<String>,
    /// Scale factor for elevs
    pub scale: f32,
    /// Offset factor for elevs
    /// actual = input*scale + offset
    pub offset: f32,
    //  Water level
    pub water_lev: f32,
}

/// Elevations as JSON data
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ElevsJson {
    /// Offset and scale for elevation data
    offset: f32,
    /// Apply scale first, then offset.
    scale: f32,
    /// Height data, a long set of hex data.  
    elevs: Vec<String>,
}

impl ElevsJson {
    /// Get elevations as numbers before offsetting.
    /// Input is a hex string representing one elev per byte.
    pub fn get_unscaled_elevs(&self) -> Result<Vec<Vec<u8>>, Error> {
        let elevs: Result<Vec<_>, _> = self.elevs.iter().map(|s| hex::decode(s)).collect();
        Ok(elevs?)
    }
}

impl UploadedRegionInfo {
    /// Default region size, used on grids that don't do varregions.
    pub const DEFAULT_REGION_SIZE: u32 = 256;

    /// Parse from string
    pub fn parse(s: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(s)?)
    }

    /// Get size, applying default region size for non-varregions
    pub fn get_size(&self) -> [u32; 2] {
        if let Some(size) = self.size {
            size
        } else {
            [Self::DEFAULT_REGION_SIZE, Self::DEFAULT_REGION_SIZE]
        }
    }

    /// Get grid in canonial lowercase format
    pub fn get_grid(&self) -> String {
        self.grid.to_lowercase()
    }

    /// Get region name in canonical lowercase format
    pub fn get_name(&self) -> String {
        self.name.to_lowercase()
    }

    /// Get elevs as a blob for SQL.
    /// Elevs are a vector of rows of hex strings at this point.
    pub fn get_elevs_as_blob(&self) -> Result<Vec<u8>, Error> {
        let elevs_blob: Vec<_> = self.get_unscaled_elevs()?.into_iter().flatten().collect();
        Ok(elevs_blob)
    }
    /// Get elevations as numbers before offsetting.
    /// Input is a hex string representing one elev per byte
    /// Output is an array of hex strings.
    pub fn get_unscaled_elevs(&self) -> Result<Vec<Vec<u8>>, Error> {
        let elevs: Result<Vec<_>, _> = self.elevs.iter().map(|s| hex::decode(s)).collect();
        Ok(elevs?)
    }

    /// Scale the elevations
    pub fn get_scaled_elevs(&self) -> Result<Vec<Vec<f32>>, Error> {
        todo!();
        //////Ok(self.get_unscaled_elevs()?.iter().map(|&v| ((v as f32) / 256.0) * self.scale + self.offset).collect())
    }
}
