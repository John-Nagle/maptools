// sculptmaker.rs
//
// Generation of Second Life terrain impostors from elevation data.
// Animats, October 2020
// License: GPL

use image::{Rgb, RgbImage, ImageReader, DynamicImage, imageops::{replace, FilterType}};
use std::cmp::{max};
use std::hash::{Hash, Hasher, DefaultHasher};
use std::f64;
use anyhow::{anyhow, Error};
use std::io::{Cursor};
use chrono::{DateTime, Utc};

/// Calculate hash for duplicate check.
fn calc_rgbimage_hash(img: &RgbImage) -> u32 {
    let mut hasher = DefaultHasher::new();
    img.hash(&mut hasher);
    let hash: u64 = hasher.finish();
    //  We only want a 32-bit hash, because we have a length problem.
    (((hash >> 32) & 0xffffffff) ^ (hash & 0xffffffff)) as u32
}

/// Sculpt textures are always 64x64, but we build them as 32x32 then double the size for legacy SL reasons.
const SCULPTDIM: usize = 32; 

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

            log::debug!("Z bounds: {:.2} to {:.2}", minz, maxz);

            let mut img = RgbImage::new(elevs.len() as u32, elevs[0].len() as u32);
            let range = maxz - minz;
            let range = range.max(0.001);   // avoid divide by 0 for flat terrain
            for x in 0..elevs.len() {
                for y in 0..elevs[0].len() {
                    //////let zscaled = (elevs[x][y] - minz) / (maxz - minz);
                    let zscaled = (elevs[x][y] - minz) / range;
                    assert!((0.0..=1.0).contains(&zscaled));
                    let zpixel = ((zscaled * 255.0).floor() as i32).clamp(0, 255) as u8;
                    let xpixel = ((x as f64 * 255.0) / (elevs.len() - 1) as f64).round().clamp(0.0, 255.0) as u8;
                    let ypixel = ((y as f64 * 255.0) / (elevs[0].len() - 1) as f64).round().clamp(0.0, 255.0) as u8;

                    // Elevs is ordered with +Y as north, but sculpt images have to be flipped in Y
                    let flipped_y = elevs[0].len() - y - 1;
                    img.put_pixel(x as u32, flipped_y as u32, Rgb([xpixel, ypixel, zpixel]));
                }
            }
            ////////  Avoid edge effects at sculplt size reduction
            //////Self::fix_sculpt_image_edges(&mut img);
            let img = Self::double_image_size(Self::add_flat_sides(&img));
            self.image = Some(img);
        }
    }
/*    
    /// Fix sculpt image edges.
    /// The outer edges must have uniform values for the two edge pixels.
    /// Sculpts are reduced to 32x32 within viewers, and that reduction is somewhat strange.
    pub fn fix_sculpt_image_edges(img: &mut RgbImage) {
        //  Fix top row and bottom row, one row in from the edge.
        //  Put pixel 0 into pixel 1.
        //  If img is 64 high/wide, we want to get pixel 63 and put it in pixel 62.
        assert!(img.width() > 2);
        assert!(img.height() > 2);
        for x in 0..img.width() {
            img.put_pixel(x, 1, *img.get_pixel(x, 0));
            img.put_pixel(x, img.height()-2, *img.get_pixel(x, img.height()-1)); 
        }
        //  Fix left edge and right edge. Make 1 in from edge match the edge.
        for y in 0..img.height() {
            img.put_pixel(1, y, *img.get_pixel(0, y));
            img.put_pixel(img.width()-2, y, *img.get_pixel(img.width()-1, y)); 
        }
    }
*/
    
    /// Double image size
    fn double_image_size(img: RgbImage) -> RgbImage {
        //  Double image size, because our 32x32 needs to be a 64x64 for the sculpt system.
        //  The sculpt system will turn it back into a 32x32. Yes, the way that works is silly.
        let width = img.width()*2;
        let height = img.height()*2;
        //  This is resizing an elevation map, not an RGB image. Do not filter.
        DynamicImage::resize_exact(&img.into(), width, height, FilterType::Nearest).into()       
    }
    
    /// Add a row and column at the edge to bring the Z value down to 0 at the edge.
    /// This gives the map tile flat vertical sides
    fn add_flat_sides(old_img: &RgbImage) -> RgbImage {
        //  Create copy with original image centered between extra rows and cols.
        let mut img = RgbImage::new(old_img.width()+2, old_img.height()+2);
        replace(&mut img, old_img, 1, 1);
        //  Zero out the Z coordinate
        let zero_z = |p: Rgb<u8>| Rgb([p[0], p[1], 0]);
        //  Create the edge rows and columns
        for x in 0..img.width() {
            img.put_pixel(x, 0, zero_z(*img.get_pixel(x, 1)));
            img.put_pixel(x, img.height()-1, zero_z(*img.get_pixel(x, img.height()-2))); 
        }
        //  Fix left edge and right edge. Make 1 in from edge match the edge.
        for y in 0..img.height() {
            img.put_pixel(0, y, zero_z(*img.get_pixel(1, y)));
            img.put_pixel(img.width()-1, y, zero_z(*img.get_pixel(img.width()-2, y))); 
        }
        img
    }
    
    /// Get uniqueness hash
    pub fn get_hash(&self) -> Result<u32, Error> {
        Ok(calc_rgbimage_hash(self.image.as_ref().unwrap()))
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
    /// Last modified timestamp
    pub last_modified: Option<DateTime<Utc>>,
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
            last_modified: None,
        }
    }
    
    /// Actually makes the image and stores it in Self.
    /// Temporary dumb version - just gets what the SL map has.
    /// Need to generate our own larger images.
    /// Need to add ability to adjust resolution.
    pub fn makeimage(&mut self, _resolution: u32) -> Result<(), Error> {
        //  ***NEED TO GET OS PREFIX FROM - WHERE? ***
        const URL_PREFIX: &str = "https://secondlife-maps-cdn.akamaized.net/map-";
        let (img, last_modified_str) = Self::fetch_terrain_image(URL_PREFIX, self.region_coords_x, self.region_coords_y, self.lod)?;
        let last_modified = DateTime::parse_from_rfc2822(&last_modified_str)?.with_timezone(&Utc);
        log::debug!("Image last modified at {:?}", last_modified);
        const PERIMETER_PIXELS: u32 = 1;
        let img = Self::add_perimeter_to_image(img, PERIMETER_PIXELS);
        self.image = Some(img.into());
        self.last_modified = Some(last_modified);
        Ok(())
    }
    
    /// Get uniqueness hash
    pub fn get_hash(&self) -> Result<u32, Error> {
        Ok(calc_rgbimage_hash(self.image.as_ref().unwrap()))
    }
    
    /// Shrink image by specified amount on each edge.
    /// This has to match what we do to the sculpts, so that
    /// the folded-down edges will work.
    pub fn add_perimeter_to_image(mut img: DynamicImage, shrink_pixels: u32) -> DynamicImage {
        let inner_img = DynamicImage::resize_exact(&img, img.width() - 2*shrink_pixels, img.height() - 2*shrink_pixels, FilterType::CatmullRom);
        replace(&mut img, &inner_img, shrink_pixels.into(), shrink_pixels.into());
        img
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
        lod: u8) -> Result<(DynamicImage, String), Error> {
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
        log::debug!("Fetching URL: {}", url);  
        let mut resp = ureq::get(&url)
            //////.set("User-Agent", USERAGENT)
            .header("Content-Type", "image/jpg") // 
            .call()
            .map_err(anyhow::Error::msg)?;
            //////.with_context(|| format!("Reading map tile  {}", url))?;
        //////let content_type = resp.headers().get("Content-Type").ok_or_else(|| anyhow!("No content type for image fetch"))?;
        //  Get last_modified time, used to disambiguate problems with cache servers.
        let last_modified = resp.headers().get("Last-Modified")
            .ok_or_else(|| anyhow!("No Last-Modified time for image fetch"))?
            .to_str()?
            .to_string();
        let raw_data = resp.body_mut().read_to_vec()?;     
        let reader = ImageReader::new(Cursor::new(raw_data))
            .with_guessed_format()
            .expect("Cursor io never fails");
        //////assert_eq!(reader.format(), Some(ImageFormat::Pnm));

        let image: DynamicImage = reader.decode()?;
        Ok((image, last_modified))
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
