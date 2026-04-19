// fetchtextures.rs
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
use common::{RegionData, TerrainGeometry, TileType, get_with_retry};
use ureq::Agent;


/// Make a texture for a terrain sculpt.
/// This is, for now, just the ground texture from the map tile server.
#[derive(Clone)]
pub struct TerrainSculptTexture {
    /// The region data
    region_data: RegionData,
    /// Last modified timestamp
    pub last_modified: Option<DateTime<Utc>>,
    /// Generated image
    pub image: Option<RgbImage>,
    
}

impl TerrainSculptTexture {
    //  SL only - needs work.
    const URL_PREFIX: &str = "https://secondlife-maps-cdn.akamaized.net/map-";
    /// Usual new, doesn't do any real work
    pub fn new(region_data: &RegionData) -> Self {
        Self {
            region_data: region_data.clone(),
            image: None,
            last_modified: None,
        }
    }
}

/// Texture fetching
pub struct FetchTextures {
    /// URL prefix for server access
    url_prefix: String,
}

impl FetchTextures {
    //  SL only - needs work.
    const URL_PREFIX: &str = "https://secondlife-maps-cdn.akamaized.net/map-";
    /// Usual new, doesn't do any real work
    pub fn new() -> Self {
        Self {
            url_prefix: Self::URL_PREFIX.to_string(),
        }
    }
        
    /// Fetch terrain image.
    /// We can get terrain images from the map servers of SL and OS.
    /// Level 0 LOD items are already in the SL asset store and have a UUID,
    /// but there's no easy way to get that UUID without a viewer. So
    /// we have to duplicate them in asset storage.
    ///
    /// Current SL official API:
    /// https://secondlife-maps-cdn.akamaized.net/map-1-1024-1024-objects.jpg
    /// This is currently SL ONLY.
    pub fn fetch_terrain_image_single(
        &self,
        texture: &TerrainSculptTexture,
        agent: &mut Agent) -> Result<(DynamicImage, DateTime<Utc>), Error> {
        const STANDARD_TILE_SIZE: u32 = 256; // Even on OS
        let tile_id_x = texture.region_data.region_loc_x / STANDARD_TILE_SIZE;
        let tile_id_y = texture.region_data.region_loc_y / STANDARD_TILE_SIZE;
        let lod = texture.region_data.lod as u32;
        assert!(lod <= 7);              // SL limit
        let region_loc_x = texture.region_data.region_loc_x;
        let region_loc_y = texture.region_data.region_loc_y;
        if region_loc_x % STANDARD_TILE_SIZE * lod.pow(2) != 0
        || region_loc_y % STANDARD_TILE_SIZE * lod.pow(2) != 0 {
            return Err(anyhow!("Terrain image location ({},{}) lod {} is invalid.", 
                region_loc_x, region_loc_y, lod));
        }
        const URL_SUFFIX: &str = "-objects.jpg"; // make sure this is the same for OS
        let url = format!("{}{}-{}-{}{}", self.url_prefix, lod + 1, tile_id_x, tile_id_y, URL_SUFFIX);
        log::debug!("Fetching URL: {}", url);  
        let mut resp = get_with_retry(agent, &url)?;
        //  Get last_modified time, used to disambiguate problems with cache servers.
        let last_modified_str = resp.headers().get("Last-Modified")
            .ok_or_else(|| anyhow!("No Last-Modified time for image fetch"))?
            .to_str()?
            .to_string();
        let last_modified = DateTime::parse_from_rfc2822(&last_modified_str)?.with_timezone(&Utc);
        let raw_data = resp.body_mut().read_to_vec()?;     
        let reader = ImageReader::new(Cursor::new(raw_data))
            .with_guessed_format()
            .expect("Cursor io never fails");
        //////assert_eq!(reader.format(), Some(ImageFormat::Pnm));

        let image: DynamicImage = reader.decode()?;
        Ok((image, last_modified))
    }
        
    /// For LODs beyond 8, the image is not available from the map API, and we have to construct it.
    pub fn fetch_terrain_image(
        &self,
        texture: &TerrainSculptTexture,
        agent: &mut Agent,
        ) -> Result<(DynamicImage, DateTime<Utc>), Error> {
        const MAX_IMAGE_SIZE_GENERATED: u32 = 1024;   // generate no images bigger than this
        assert!(texture.region_data.lod < 15);  // sanity
        if texture.region_data.lod <= 7 {
            self.fetch_terrain_image_single(texture, agent)
        } else {
            //  Request four images and combine.
            let half_size_x = texture.region_data.region_size_x / 2;
            let half_size_y = texture.region_data.region_size_y / 2;
            let half_lod = texture.region_data.lod - 1;            
            let mut fetch = |lod, dx, dy| {
                let mut half_terrain_image = texture.clone();
                half_terrain_image.region_data.region_loc_x = dx;
                half_terrain_image.region_data.region_loc_y = dy;
                half_terrain_image.region_data.lod = half_lod;
                log::debug!("Multi region image needed for LOD #{}: offset ({},{})", lod, dx, dy);  // ***TEMP***
                self.fetch_terrain_image(&half_terrain_image, agent)
            };
            //  Get the four images.
            //  Region size here is the full sized impostor, so we have to divide by 2 to get the size of the 4 squares that make it up.
            let images = [
                fetch(half_lod, 0, 0)?,            
                fetch(half_lod, half_size_x, 0)?,
                fetch(half_lod, 0, half_size_y)?,
                fetch(half_lod, half_size_x, half_size_y)?
                ];

            //  ***MORE*** works like the sculpt LOD system.
            let (mut quad_image, last_modified) = Self::combine_terrain_images(images);
            if quad_image.width() > MAX_IMAGE_SIZE_GENERATED || quad_image.height() > MAX_IMAGE_SIZE_GENERATED {
                let half_width = quad_image.width() / 2;
                let half_height = quad_image.height() / 2;
                quad_image = quad_image.resize(half_width, half_height, FilterType::Gaussian);
            }
            //  ***NEED TO DOWNSIZE IMAGE IF TOO BIG***
            Ok((quad_image, last_modified))
        }
    }
    
    /// Combine 4 terrain images.
    /// Input order is lower left, lower right, uppler left, upper right.
    /// All images must be the same size.
    /// The output image is twice as big.
    fn combine_terrain_images(images: [(DynamicImage, DateTime<Utc>);4]) -> (DynamicImage, DateTime<Utc>) {
        let w = images[0].0.width();
        let h = images[0].0.height();
        const OFFSETS: [(u32, u32);4] = [(0, 0), (1, 0), (0, 1), (1, 1)];
        let mut img = DynamicImage::new_rgb8((w*2).into(), (h*2).into());
        let mut last_modified = images[0].1;
        for n in 0..3 {
            assert_eq!(images[n].0.width(), w);
            assert_eq!(images[n].0.height(), h);
            replace(&mut img, &images[n].0, (OFFSETS[n].0*w).into(), (OFFSETS[n].1*h).into());
            last_modified = last_modified.max(images[n].1);
        }
        //  Last modified date is latest date
        (img, last_modified)
    }
}

#[test]
fn fetch_terrain_texture() {
    use std::time::Duration;
    //  Want logging, but need to turn off Trace level to avoid too much junk.
    let _ = simplelog::CombinedLogger::init(
        vec![
            simplelog::TermLogger::new(simplelog::LevelFilter::Debug, simplelog::Config::default(), simplelog::TerminalMode::Stdout, simplelog::ColorChoice::Auto),]
    );
    
    const TIMEOUT_CONNECT: Duration = Duration::from_secs(15);
    const TIMEOUT_GLOBAL: Duration = Duration::from_secs(120);
    //  HTTP connection pool, used to validate UUIDs against asset server.
    let config = Agent::config_builder()
        .timeout_connect(Some(TIMEOUT_CONNECT))
        .timeout_global(Some(TIMEOUT_GLOBAL))
        .user_agent(crate::TERRAIN_GENERATOR_USER_AGENT)
        .build();
    let mut agent: Agent = config.into();

    let region_data = RegionData {
        region_loc_x: 1000*256,
        region_loc_y: 1000*256,
        region_size_x: 256,
        region_size_y: 256,
        lod: 0,
        grid: "agni".to_string(),
        name: "Da Boom".to_string(),
    };
    //////let img = TerrainSculptTexture::fetch_terrain_image(URL_PREFIX, 1000*256, 1000*256, 0).expect("Terrain fetch failed");
    let fetch_textures = FetchTextures::new();
    let terrain_sculpt_texture = TerrainSculptTexture::new(&region_data);
    let img = fetch_textures.fetch_terrain_image(&terrain_sculpt_texture, &mut agent).expect("Terrain fetch failed");
    img.0.save("/tmp/testimg.jpg").expect("test image write failed");
}
