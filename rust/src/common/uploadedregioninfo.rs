//! Access to raw terrain table in SQL.
//!
//! License: LGPL.
//! Animats
//! August, 2025.
//
use anyhow::{Error, anyhow};
use serde::Deserialize;
use array2d::{Array2D};
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
    pub elevs: Vec<String>,
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
    
    /// Usual new. This takes elevations as hex strings.
    pub fn new(grid: String, region_coords_x: u32, region_coords_y: u32, size_x: u32, size_y: u32, name: String, elevs: Vec<String>, scale: f32, offset: f32, water_lev: f32) -> Self {
        Self {
            grid,
            region_coords: [region_coords_x, region_coords_y],
            size: Some([size_x, size_y]),
            name,
            elevs,
            scale,
            offset,
            water_lev,
        }               
    }

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
    
    /// Get dimensions of elevation samples array
    pub fn get_samples(&self) -> Result<[u32;2], Error> {
        if self.elevs.is_empty() {
            return Err(anyhow!("Elevation data is missing"));
        }
        //  Validate that all rows are the same length
        let rowlen = self.elevs[0].len()/2;  // it's a hex string, we want the byte count
        for row in &self.elevs {
            if row.len() != rowlen*2 {
                return Err(anyhow!("Elevation data has a row of the wrong length. Not {}", rowlen));
            }
        } 
        Ok([self.elevs.len().try_into()?, rowlen.try_into()?])
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
    
    /// Convert SQL blob to hex format.
    /// We have to figure out the length of the strings from the length and aspect ratio.
    pub fn elevs_blob_to_hex(elevs: Vec<u8>, size_x: u32, size_y: u32) -> Result<Vec<String>, Error> {
        let n = elevs.len() as u32;
        let gcd = num::integer::gcd(size_x, size_y) as u32;
        let sx = size_x / gcd;
        let sy = size_y / gcd;
        if n % (sx*sy) != 0 {
            return Err(anyhow!("Elevation data size incorrect: length {}, size ({}, {})", n, size_x, size_y));
        }
        let r = n / (sx*sy);
        let elevs_x = size_x / r;
        let elevs_y = size_y / r;
        assert_eq!(n, elevs_x * elevs_y);
        //  Now take slices of length elevs_x and make into hex.
        Ok(elevs.chunks_exact(elevs_x as usize).map(|c|  hex::encode_upper(c)).collect())
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

/// Height field.
/// Always an odd number of rows and columns, because the right and top edges
/// are supposed to be the edges adjacent regions.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightField {
    /// The heights
    heights: Array2D<f32>,
    /// size of region, X
    pub size_x: u32,
    /// size of region, Y
    pub size_y: u32,
}

impl std::fmt::Display for HeightField {
    /// Usual display
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "HeightField samples ({}, {})  region ({}, {})", self.heights.num_rows(), self.heights.num_columns(), self.size_x, self.size_y)
    }
}

//  ***CHECK COLUMN/ROW ORDER***
impl HeightField {
    /// New from elevs blob, the form used in SQL. One big blob, a flattened 2D array.
    /// size_x and size_y are size of the region, not the elevs data.
    pub fn new_from_elevs_blob(elevs: &Vec<u8>, samples_x: u32, samples_y: u32, size_x: u32, size_y: u32, scale: f32, offset: f32) -> Result<Self, Error> {
        if elevs.len() != (samples_x as usize) * (samples_y as usize) {
            return Err(anyhow!("Elevations array data length {} does not match dimensions ({}, {})", 
                elevs.len(), samples_x, samples_y));
        }
        let iterator = (0..).map(|n| ((elevs[n] as f32) / 256.0) * scale + offset);
        let heights = Array2D::from_iter_column_major(iterator, samples_x as usize, samples_y as usize)?;
        Ok(Self {
            heights, 
            size_x,
            size_y,
        })
    }
    
    /// New from the 2D array of elevs we get from JSON
    pub fn new_from_unscaled_elevs(elevs: &Vec<Vec<u8>>, size_x: u32, size_y: u32, scale: f32, offset: f32) -> Result<Self, Error> {  
        if elevs.is_empty() {
            return Err(anyhow!("Elevs array is empty."));
        }
        let row_length = elevs[0].len();
        let iterator = (0..).map(|n| {
            let x = n % row_length;
            let y = n / row_length;
            ((elevs[x][y] as f32) / 256.0) * scale + offset });
        let heights = Array2D::from_iter_row_major(iterator, row_length, elevs.len())?;
        Ok(Self {
            heights, 
            size_x,
            size_y,
        })
    }
}

#[test]
/// Test height field column organization
fn test_height_field() {
    println!("Test height field.");
    let flattened: Vec<u8> = vec![0u8, 1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8, 8u8];
    let arrayform: Vec<Vec<u8>> = vec![vec![0u8, 1u8, 2u8], vec![3u8, 4u8, 5u8], vec![6u8, 7u8, 8u8]];
    let hf_flat = HeightField::new_from_elevs_blob(&flattened, 3, 3, 256, 256, 256.0, 0.0).expect("New from blob failed");
    let hf_arrayform = HeightField::new_from_unscaled_elevs(&arrayform, 256, 256, 256.0, 0.0).expect("New from unsscaled elevs failed");
    println!("hf_flat: {:?}", hf_flat);
    println!("hf_arrayform: {:?}", hf_arrayform);
    assert_eq!(hf_flat, hf_arrayform);
}
