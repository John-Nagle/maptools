// sculptmaker.rs
//
// Generation of Second Life terrain impostors from elevation data.
// Animats, October 2020
// License: GPL

use image::{Rgb, RgbImage};
//////use serde::Deserialize;
//////use serde_json;
use std::cmp::{max, min};
//////use std::env;
use std::f64;
//////use std::fs::File;
//////use std::io::{BufReader, Read};
//////use std::path::Path;
//////use std::process;

const SCULPTDIM: usize = 64; // Sculpt textures are always 64x64

#[derive(Debug)]
pub struct TerrainSculpt {
    //////region: String,
    pub image: Option<RgbImage>,
    elevs: Option<Vec<Vec<f64>>>,
    zheight: Option<f64>,
    zoffset: Option<f64>,
    //////waterheight: Option<f64>,
}

impl TerrainSculpt {
    pub fn new(_region: &str) -> Self {
        TerrainSculpt {
            //////region: region.to_string(),
            image: None,
            elevs: None,
            zheight: None,
            zoffset: None,
            //////waterheight: None,
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
/*
fn unpackelev(elev: &str) -> Vec<u8> {
    (0..(elev.len() / 2))
        .map(|n| u8::from_str_radix(&elev[n * 2..n * 2 + 2], 16).unwrap())
        .collect()
}

// Struct to match JSON input
#[derive(Deserialize)]
struct TerrainJson {
    elevs: Vec<String>,
    scale: f64,
    offset: f64,
    region: String,
}

fn handlefile(filename: &str, outprefix: &str) {
    // Read file
    let mut file = File::open(filename).expect("Unable to open file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Unable to read file");

    let pos = contents.find("\n{").unwrap_or(0);
    if pos < 1 {
        panic!("Unable to find JSON data in file \"{}\"", filename);
    }
    let s = &contents[(pos - 1)..];

    let jsn: TerrainJson = serde_json::from_str(s).expect("JSON parsing failed");
    let elevs: Vec<Vec<u8>> = jsn.elevs.iter().map(|row| unpackelev(row)).collect();

    println!(
        "Region: {} scale: {:.3} offset {:.3}",
        jsn.region, jsn.scale, jsn.offset
    );

    let mut sculpt = TerrainSculpt::new(&jsn.region);
    sculpt.setelevs(elevs, jsn.scale, jsn.offset);
    sculpt.makeimage();

    if let Some(img) = sculpt.image {
        let outfile = format!("{}{}.png", outprefix, jsn.region);
        img.save(&outfile).expect("Failed to save image");
        println!("Saved {}", outfile);
    }
}

fn main() {
    let outprefix = "/tmp/terrainsculpt-";
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <filenames>", args[0]);
        process::exit(1);
    }
    let filenames = &args[1..];
    for filename in filenames {
        handlefile(filename, outprefix);
    }
}

// For unit test, you can add
// fn testmain() {
//     let fname = "/tmp/sculpttest.png";
//     let mut sculpt = TerrainSculpt::new("test");
//     sculpt.pyramidtest();
//     sculpt.makeimage();
//     if let Some(img) = sculpt.image {
//         img.save(fname).expect("Failed to save image");
//         println!("Wrote {}", fname);
//     }
// }
*/
