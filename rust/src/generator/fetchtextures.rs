// fetchtextures.rs
//
// Generation of Second Life terrain impostors from elevation data.
// Animats, October 2020
// License: LGPL

use image::{ImageReader, DynamicImage, imageops::{replace, FilterType}};
use anyhow::{anyhow, Error};
use std::io::{Cursor};
use std::rc::{Rc};
use chrono::{DateTime, Utc};
use common::{RegionData, get_with_retry};
use ureq::Agent;
use std::collections::{HashSet};
use cached::{Cached, SizedCache};

/// Put DynamicImage inside an Rc to reduce copies.
pub type RcDynamicImage = Rc<DynamicImage>;
/// Local short key for sets of tiles that have land. ((X, Y), LOD)
type TileKey = ((u32, u32), u8);
/// Largest possible LOD before things overflow. Never gets this big.
const MAX_LOD: u8 = 12;

/// Texture fetching
pub struct FetchTextures {
    /// URL prefix for server access
    url_prefix: String,
    /// Agent - for HTTP requests
    agent: Agent,
    /// Region size (optional) - size of all regions, if homogeneous.
    /// The LOD system only works for  groups with homogeneous regions.
    #[allow(dead_code)]
    region_size_opt: Option<(u32, u32)>,
    /// Valid regions in this vizgroup. Never fetch anything not in this set.
    tiles_with_land: HashSet<TileKey>,
    /// Water image, used for blank areas
    water_images: Vec<RcDynamicImage>,
    /// Cache of already computed tiles.
    /// Without this it takes hours.
    cache: SizedCache<TileKey, (RcDynamicImage, DateTime<Utc>)>,
}

impl FetchTextures {
    //  SL only - needs work.
    const URL_PREFIX: &str = "https://secondlife-maps-cdn.akamaized.net/map-";
    //  Cache size.
    const CACHE_SIZE: usize = 1000;
    //  Generate no images bigger than this
    const MAX_IMAGE_SIZE_GENERATED: u32 = 1024;   
    /// Usual new, doesn't do any real work
    pub fn new(agent: &Agent, regions: &Vec<RegionData>, region_size_opt: Option<(u32, u32)>) -> Self {
        //  Set of valid regions, used to decide what can be fetched.
        let tiles_with_land = Self::build_tiles_with_land(regions, region_size_opt.unwrap());
        //  Water heights for all LOD0 tiles.
/*
        let water_height_items: Vec<_> = regions.iter().map(|r| WaterPoint { point: [r.region_loc_x, r.region_loc_y], water_height: r.water_height}).collect();
        let water_height_items = vec![];    // ***TEMP***
        let tile_water_heights = KdTree2::<WaterPoint>::build_by(water_height_items, 
            |item1, item2, k| item1.point[k].cmp(&item2.point[k]));
*/
        //  Fixed water image, for other areas. Loaded at compile time.
        let water_images = Self::build_water_images(region_size_opt.unwrap(), MAX_LOD);
        Self {
            url_prefix: Self::URL_PREFIX.to_string(),
            agent: agent.clone(),
            region_size_opt,
            tiles_with_land,
            //////tile_water_heights,
            water_images,
            cache: SizedCache::with_size(Self::CACHE_SIZE),
        }
    }
    
    /// Build all the water images.
    /// These are tiles used where there is no land.
    /// One for each LOD.
    fn build_water_images(region_size: (u32,u32), max_lod: u8) -> Vec<RcDynamicImage> {
        const WATER_IMAGE: &[u8] = include_bytes!("../assets/basicwater2-256-256.png");        
        let water_image = Rc::new(image::load_from_memory(WATER_IMAGE).expect("Failed to load water image"));
        //  Resize water to region size. Really ought to duplicate.
        let water_image = Rc::new(water_image.resize(region_size.0, region_size.1, FilterType::Triangle));
        let mut water_images = vec![water_image];
        for _lod in 1..max_lod {
            let prev_image = (water_images[water_images.len()-1].clone(), Utc::now());
            let (mut quad_image, _timestamp) = Self::combine_terrain_images(
                [prev_image.clone(), prev_image.clone(), prev_image.clone(), prev_image.clone()]);
            if quad_image.width() > Self::MAX_IMAGE_SIZE_GENERATED || quad_image.height() > Self::MAX_IMAGE_SIZE_GENERATED {
                let half_width = quad_image.width() / 2;
                let half_height = quad_image.height() / 2;
                quad_image = Rc::new(quad_image.resize(half_width, half_height, FilterType::Triangle));
            }
            drop(prev_image);
            water_images.push(quad_image);
        }
        water_images
    }
    
    /// Precompute which tiles have some land in them.
    fn build_tiles_with_land(regions: &Vec<RegionData>, region_size: (u32, u32)) -> HashSet<TileKey> {
        log::info!("Building set of tiles with land start.  {} regions.", regions.len());
        let mut all_tiles_with_land: HashSet<TileKey> = regions.iter().map(|r| ((r.region_loc_x, r.region_loc_y), 0)).collect();
        let mut working_tiles = all_tiles_with_land.clone();
        //  Iterate over the LODs. 
        //  Generate the next lower LOD by marking every tile with a land tile at a higher LOD.
        for lod in 1..MAX_LOD {
            let mut new_working_tiles:  HashSet<TileKey> = HashSet::new();
            let size_x = 2_u32.pow(lod.into()) * region_size.0;
            let size_y = 2_u32.pow(lod.into()) * region_size.1;
            for ((x, y), working_lod) in working_tiles {
                assert_eq!(lod, working_lod + 1);
                let new_tile_key = ((previous_multiple_of(x, size_x), previous_multiple_of(y, size_y)), lod);
                log::debug!("  Land tile: ({},{}), LOD #{}", new_tile_key.0.0, new_tile_key.0.1, new_tile_key.1);
                new_working_tiles.insert(new_tile_key);
                all_tiles_with_land.insert(new_tile_key);
            }
            working_tiles = new_working_tiles;
            //  We are done when there is one big tile.
            log::debug!("Built set of tiles with land: LOD #{}, size ({},{}), {} tiles.",
                lod, size_x, size_y, working_tiles.len());
            if working_tiles.len() <= 1 {
                break;
            }
        }
        log::info!("Building set of tiles with land done, {} tiles total.", all_tiles_with_land.len());
        println!("Total land tiles, all LODS: {}.", all_tiles_with_land.len());
        all_tiles_with_land
    }
    
    /// True if tile has some land. 
    /// Don't have to generate impostors for tiles with no land.
    pub fn tile_has_land(&self, key: TileKey) -> bool {
        self.tiles_with_land.contains(&key)
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
    fn fetch_terrain_image_single(
        &self,
        region_data: &RegionData) 
        -> Result<(RcDynamicImage, DateTime<Utc>), Error> {
        const STANDARD_TILE_SIZE: u32 = 256; // Even on OS
        let tile_id_x = region_data.region_loc_x / STANDARD_TILE_SIZE;
        let tile_id_y = region_data.region_loc_y / STANDARD_TILE_SIZE;
        let lod = region_data.lod as u32;
        assert!(lod <= 7);              // SL limit
        let region_loc_x = region_data.region_loc_x;
        let region_loc_y = region_data.region_loc_y;
        if region_loc_x % STANDARD_TILE_SIZE * 2_u32.pow(lod.into()) != 0
        || region_loc_y % STANDARD_TILE_SIZE * 2_u32.pow(lod.into()) != 0 {
            return Err(anyhow!("Terrain image location ({},{}) lod {} is invalid.", 
                region_loc_x, region_loc_y, lod));
        }
        const URL_SUFFIX: &str = "-objects.jpg"; // make sure this is the same for OS
        let url = format!("{}{}-{}-{}{}", self.url_prefix, lod + 1, tile_id_x, tile_id_y, URL_SUFFIX);
        log::debug!("Fetching URL: {}", url);  
        let mut resp = get_with_retry(&self.agent, &url)?;
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

        let image: RcDynamicImage = Rc::new(reader.decode()?);
        Ok((image, last_modified))
    }
        
    /// For LODs beyond 8, the image is not available from the map API, and we have to construct it.
    pub fn fetch_terrain_image(
        &mut self,
        region_data: &RegionData) 
        -> Result<(RcDynamicImage, DateTime<Utc>), Error> {        
        assert!(region_data.lod < MAX_LOD);  // sanity
        //  If region is all water, use a canned water image.
        if !self.tile_has_land(((region_data.region_loc_x, region_data.region_loc_y), region_data.lod)) {
            return self.fetch_water_image(region_data);
        }
        if region_data.lod == 0 {
            //  OK to fetch
            self.fetch_terrain_image_single(region_data)
        } else {
            let cache_key = ((region_data.region_loc_x, region_data.region_loc_y), region_data.lod);
            if let Some((cached_image, last_modified)) = self.cache.cache_get(&cache_key) {
                log::debug!("Terrain cache hit: {:?}", cache_key);
                return Ok((cached_image.clone(), last_modified.clone()))
            }
            log::debug!("Terrain cache miss: {:?}", cache_key);
            //  Request four images and combine
            let half_size_x = region_data.region_size_x / 2;
            let half_size_y = region_data.region_size_y / 2;
            let half_lod = region_data.lod - 1;            
            let mut fetch = |lod, dx, dy| {
                let mut quadrant_region_data = region_data.clone();
                quadrant_region_data.region_loc_x += dx;
                quadrant_region_data.region_loc_y += dy;
                quadrant_region_data.region_size_x = half_size_x;
                quadrant_region_data.region_size_y = half_size_y;
                quadrant_region_data.lod = half_lod;
                log::debug!("Multi region image needed for LOD #{}: offset ({},{})", lod, dx, dy);  // ***TEMP***
                self.fetch_terrain_image(&quadrant_region_data)
            };
            //  Get the four images.
            //  Region size here is the full sized impostor, so we have to divide by 2 to get the size of the 4 squares that make it up.
            let images = [
                fetch(half_lod, 0, 0)?,            
                fetch(half_lod, half_size_x, 0)?,
                fetch(half_lod, 0, half_size_y)?,
                fetch(half_lod, half_size_x, half_size_y)?
                ];

            let (mut quad_image, last_modified) = Self::combine_terrain_images(images);
            //  Downsize image if too big.
            if quad_image.width() > Self::MAX_IMAGE_SIZE_GENERATED || quad_image.height() > Self::MAX_IMAGE_SIZE_GENERATED {
                let half_width = quad_image.width() / 2;
                let half_height = quad_image.height() / 2;
                quad_image = Rc::new(quad_image.resize(half_width, half_height, FilterType::Triangle));
            }
            //  Save a cached copy
            let _ = self.cache.cache_set(cache_key, (quad_image.clone(), last_modified.clone()));
            Ok((quad_image, last_modified))
        }
    }
    
    /// Combine 4 terrain images.
    /// Input order is lower left, lower right, uppler left, upper right.
    /// All images must be the same size.
    /// The output image is twice as big.
    fn combine_terrain_images(images: [(RcDynamicImage, DateTime<Utc>);4]) -> (RcDynamicImage, DateTime<Utc>) {
        let w = images[0].0.width();
        let h = images[0].0.height();
        //  Offsets for insertion into the new larger image.
        //  Note that Y is flipped. That's because image coords go down from the top, while tile coordinates go up from the bottom.
        const OFFSETS: [(u32, u32);4] = [(0, 1), (1, 1), (0, 0), (1, 0)];
        let mut img = DynamicImage::new_rgb8((w*2).into(), (h*2).into());
        let mut last_modified = images[0].1;
        for n in 0..4 {
            assert_eq!(images[n].0.width(), w);
            assert_eq!(images[n].0.height(), h);
            replace(&mut img, &(*images[n].0), (OFFSETS[n].0*w).into(), (OFFSETS[n].1*h).into());
            last_modified = last_modified.max(images[n].1);
        }
        //  Last modified date is latest date
        (Rc::new(img), last_modified)
    }
    
    /// Not a location in this vizgroup. Fetch a standard water image.
    fn fetch_water_image(&self, region_data: &RegionData) -> Result<(RcDynamicImage, DateTime<Utc>), Error> {
        log::debug!("Using water image for tile {:?}", region_data);
        Ok((self.water_images[region_data.lod as usize].clone(), Utc::now()))
    }
}

/// Calculates the largest value less than or equal to n that is a multiple of rhs
fn previous_multiple_of(n: u32, rhs: u32) -> u32 {
    (n / rhs) * rhs
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
    let agent: Agent = config.into();

    let region_data = RegionData {
        region_loc_x: 1000*256,
        region_loc_y: 1000*256,
        region_size_x: 256,
        region_size_y: 256,
        lod: 0,
        grid: "agni".to_string(),
        name: "Da Boom".to_string(),
    };
    let fetch_textures = FetchTextures::new(&agent, &vec![region_data.clone()], Some((256, 256))  );
    let img = fetch_textures.fetch_terrain_image(&region_data).expect("Terrain fetch failed");
    img.0.save("/tmp/testimg.jpg").expect("test image write failed");
}
