// sculptmaker.rs
//
// Generation of Second Life terrain impostors from elevation data.
// Animats, October 2020
// License: GPL

use image::{Rgb, RgbImage, ImageReader, DynamicImage};
use std::cmp::{max, min};
use std::hash::{Hash, Hasher, DefaultHasher};
use std::f64;
use anyhow::{anyhow, Error};
use std::io::{Cursor};

/// Calculate hash for duplicate check.
fn calc_rgbimage_hash(img: &RgbImage) -> u64 {
    let mut hasher = DefaultHasher::new();
    img.hash(&mut hasher);
    hasher.finish()
}

const SCULPTDIM: usize = 64; // Sculpt textures are always 64x64

#[derive(Debug)]
pub struct TerrainSculpt {
    pub image: Option<RgbImage>,
    elevs: Option<Vec<Vec<f64>>>,
    zheight: Option<f64>,
    zoffset: Option<f64>,
}

impl TerrainSculpt {
    pub fn new(_region: &str) -> Self {
        TerrainSculpt {
            image: None,
            elevs: None,
            zheight: None,
            zoffset: None,
        }
    }

    pub fn makeimage(&mut self) {
        if let Some(elevs) = &self.elevs {
            let maxz = elevs.iter().flatten().cloned().fold(f64::MIN, f64::max);
            let minz = elevs.iter().flatten().cloned().fold(f64::MAX, f64::min);
            self.zheight = Some(maxz - minz);
            self.zoffset = Some(minz);

            println!("Z bounds: {:.2} to {:.2}", minz, maxz);

            let mut img = RgbImage::new(elevs.len() as u32, elevs[0].len() as u32);
            let range = maxz - minz;
            let range = range.max(0.001);   // avoid divide by 0 for flat terrain
            for x in 0..elevs.len() {
                for y in 0..elevs[0].len() {
                    //////let zscaled = (elevs[x][y] - minz) / (maxz - minz);
                    let zscaled = (elevs[x][y] - minz) / range;
                    assert!(zscaled >= 0.0 && zscaled <= 1.0);
                    let zpixel = max(0, min(255, (zscaled * 256.0).floor() as i32)) as u8;
                    let xpixel = ((x as f64 * 256.0) / elevs.len() as f64).round() as u8;
                    let ypixel = ((y as f64 * 256.0) / elevs[0].len() as f64).round() as u8;

                    // Elevs is ordered with +Y as north, but sculpt images have to be flipped in Y
                    let flipped_y = elevs[0].len() - y - 1;
                    img.put_pixel(x as u32, flipped_y as u32, Rgb([xpixel, ypixel, zpixel]));
                }
            }
            self.image = Some(img);
        }
    }
    
    /// Get uniqueness hash
    pub fn get_hash(&self) -> Result<u64, Error> {
        Ok(calc_rgbimage_hash(&self.image.as_ref().unwrap()))
    }

    pub fn setelevs(&mut self, elevs: Vec<Vec<u8>>, inputscale: f64, inputoffset: f64) {
        if elevs.len() == SCULPTDIM && elevs[0].len() == SCULPTDIM {
            // Directly convert to f64
            let elevs_f64: Vec<Vec<f64>> = elevs
                .into_iter()
                .map(|row| row.into_iter().map(|z| z as f64).collect())
                .collect();
            self.elevs = Some(elevs_f64);
            return;
        }
        // Interpolate to SCULPTDIM x SCULPTDIM
        let mut newelevs: Vec<Vec<f64>> = vec![vec![0.0; SCULPTDIM]; SCULPTDIM];
        let orig_x = elevs.len();
        let orig_y = elevs[0].len();

        for x in 0..SCULPTDIM {
            for y in 0..SCULPTDIM {
                let xfract = ((x as f64) / SCULPTDIM as f64) * orig_x as f64;
                let yfract = ((y as f64) / SCULPTDIM as f64) * orig_y as f64;
                let xfract = xfract.min((orig_x - 1) as f64);
                let yfract = yfract.min((orig_y - 1) as f64);

                let x0 = xfract.floor() as usize;
                let x1 = xfract.ceil() as usize;
                let y0 = yfract.floor() as usize;
                let y1 = yfract.ceil() as usize;

                let z0 = elevs[x0][y0];
                let z1 = elevs[x0][y1];
                let z2 = elevs[x1][y0];
                let z3 = elevs[x1][y1];

                let z = max(z0, max(z1, max(z2, z3))) as f64;
                newelevs[x][y] = z * (inputscale / 256.0) + inputoffset;
            }
        }
        self.elevs = Some(newelevs);
    }

    fn _pyramidtest(&mut self) {
        let mut elevs = vec![vec![0.0; SCULPTDIM]; SCULPTDIM];
        let halfway = (SCULPTDIM as f64) * 0.5;
        for x in 0..SCULPTDIM {
            for y in 0..SCULPTDIM {
                let z1 = halfway - ((halfway - x as f64).abs());
                let z2 = halfway - ((halfway - y as f64).abs());
                let z = (z1.min(z2)) / halfway;
                elevs[x][y] = z;
            }
        }
        self.elevs = Some(elevs);
    }
}

/// Make a texture for a terrain sculpt.
/// This is, for now, just the ground texture from the map tile server.
pub struct TerrainSculptTexture {
    /// Coords X and Y. Meters.
    region_coords_x: u32,
    region_coords_y: u32,
    lod: u8,
    /// Generated image
    pub image: Option<RgbImage>,
    
}

impl TerrainSculptTexture {
    /// Usual new, doesn't do any real work
    pub fn new(region_coords_x: u32, region_coords_y: u32, lod: u8, _texture_name: &str) -> Self {
        Self {
            region_coords_x,
            region_coords_y,
            lod,
            image: None,
        }
    }
    
    /// Actually makes the image and stores it in Self.
    /// Temporary dumb version - just gets what the SL map has.
    /// Need to generate our own larger images.
    /// Need to add ability to adjust resolution.
    pub fn makeimage(&mut self, _resolution: u32) -> Result<(), Error> {
        //  ***NEED TO GET OS PREFIX FROM - WHERE? ***
        const URL_PREFIX: &str = "https://secondlife-maps-cdn.akamaized.net/map-";
        let img = Self::fetch_terrain_image(URL_PREFIX, self.region_coords_x, self.region_coords_y, self.lod)?;
        self.image = Some(img.into());
        Ok(())
    }
    
    /// Get uniqueness hash
    pub fn get_hash(&self) -> Result<u64, Error> {
        Ok(calc_rgbimage_hash(&self.image.as_ref().unwrap()))
    }
    
    /// Fetch terrain image.
    /// We can get terrain images from the map servers of SL and OS.
    /// Level 0 LOD items are already in the SL asset store and have a UUID,
    /// but there's no easy way to get that UUID without a viewer. So
    /// we have to duplicate them in asset storage.
    ///
    /// Current SL official API:
    /// https://secondlife-maps-cdn.akamaized.net/map-1-1024-1024-objects.jpg
    pub fn fetch_terrain_image(
        url_prefix: &str,
        region_coords_x: u32,
        region_coords_y: u32,
        lod: u8) -> Result<DynamicImage, Error> {
        const STANDARD_TILE_SIZE: u32 = 256; // Even on OS
        let tile_id_x = region_coords_x / STANDARD_TILE_SIZE;
        let tile_id_y = region_coords_y / STANDARD_TILE_SIZE;
        let lod = lod as u32;
        if region_coords_x % STANDARD_TILE_SIZE * lod.pow(2) != 0
        || region_coords_y % STANDARD_TILE_SIZE * lod.pow(2) != 0 {
            return Err(anyhow!("Terrain image location ({},{}) lod {} is invalid.", 
                region_coords_x, region_coords_y, lod));
        }
        const URL_SUFFIX: &str = "-objects.jpg"; // make sure this is the same for OS
        let url = format!("{}{}-{}-{}{}", url_prefix, lod + 1, tile_id_x, tile_id_y, URL_SUFFIX);
        println!("URL: {}", url);   // ***TEMP***
        let mut resp = ureq::get(&url)
            //////.set("User-Agent", USERAGENT)
            .header("Content-Type", "image/jpg") // 
            .call()
            .map_err(anyhow::Error::msg)?;
            //////.with_context(|| format!("Reading map tile  {}", url))?;
        //////let content_type = resp.headers().get("Content-Type").ok_or_else(|| anyhow!("No content type for image fetch"))?;
        let raw_data = resp.body_mut().read_to_vec()?;     
        let reader = ImageReader::new(Cursor::new(raw_data))
            .with_guessed_format()
            .expect("Cursor io never fails");
        //////assert_eq!(reader.format(), Some(ImageFormat::Pnm));

        let image = reader.decode()?;
        Ok(image)
    }
}

#[test]
fn read_terrain_texture() {
    //  Want logging, but need to turn off Trace level to avoid too much junk.
    let _ = simplelog::CombinedLogger::init(
        vec![
            simplelog::TermLogger::new(simplelog::LevelFilter::Debug, simplelog::Config::default(), simplelog::TerminalMode::Stdout, simplelog::ColorChoice::Auto),]
    );

    const URL_PREFIX: &str = "https://secondlife-maps-cdn.akamaized.net/map-";
    let img = TerrainSculptTexture::fetch_terrain_image(URL_PREFIX, 1024*256, 1024*256, 0).expect("Terrain fetch failed");
    img.save("/tmp/testimg.jpg").expect("test image write failed");
}
