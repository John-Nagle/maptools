//! tileassetsgc.rs -- garbage collector for tile assets
//!
//! Remove entries from tile_assets table that do not appear in region_impostors table.
//!
//! Part of the Animats impostor system
//!
//!
//!     License: LGPL.
//!     Animats
//!     April, 2026.
//
use anyhow::{Error, anyhow};
use crate::{RegionImpostorFaceData, TileAssetType};
use mysql::prelude::{Queryable};
use mysql::{PooledConn, Row, params, Transaction, TxOpts};
//////use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One usage of a UUID in region_impostors.
/// This should be small, because we read in
/// all the assets for one grid.
#[derive(Debug)]
struct UuidUsage {
    /// What asset is being used for
    pub asset_type: TileAssetType,
    /// Asset hash
    pub asset_hash: String,
    /// Region location (meters)
    pub region_loc: [u32;2],
    /// Region size (meters)
    pub region_size: [u32;2],
    /// Asset UUID
    pub asset_uuid: Uuid
}

impl UuidUsage {
    ///  From MySQL row
    ///  Row format is:
    ///     region_loc_x, region_loc_y, region_size_x, region_size_y,
    ///     sculpt_uuid, sculpt_hash, mesh_uuid, mesh_hash,
    ///     faces_json        
    fn from_row(row_result: Result<Row, mysql::Error>) -> Result<Vec<Self>, Error> {
        let row: Row = row_result?;
        //  Decompose SQL result
        let region_loc: [u32;2] = [row.get_opt(0).ok_or_else(|| anyhow!("loc_x is null"))??, row.get_opt(1).ok_or_else(|| anyhow!("loc_y is null"))??];
        let region_size: [u32;2] = [row.get_opt(2).ok_or_else(|| anyhow!("size_x is null"))??, row.get_opt(3).ok_or_else(|| anyhow!("size_y is null"))??];
        let sculpt_uuid = convert_uuid(row.get_opt(4).ok_or_else(|| anyhow!("sculpt_uuid is invalid"))??,);
        let sculpt_hash = convert_hash(row.get_opt(5).ok_or_else(|| anyhow!("sculpt_hash is invalid"))??,);
        let mesh_uuid = convert_uuid(row.get_opt(6).ok_or_else(|| anyhow!("mesh_uuid is invalid"))??,);
        let mesh_hash = convert_hash(row.get_opt(7).ok_or_else(|| anyhow!("mesh_hash is invalid"))??,);
        let faces_json: String = row.get_opt(8).ok_or_else(|| anyhow!("faces_json is null"))??;
        let faces: Vec<RegionImpostorFaceData> = serde_json::from_str(&faces_json)?;
        let mut results = Vec::new();
        let mut add_uuid = |asset_type, asset_uuid, asset_hash_opt| {
            if let Some(asset_hash) = asset_hash_opt {
                results.push(UuidUsage {
                    asset_type,
                    asset_uuid,
                    asset_hash,
                    region_loc,
                    region_size,
                })
            };                
        };
        if let Some(sculpt_uuid) = sculpt_uuid {
            add_uuid(TileAssetType::SculptTexture, sculpt_uuid, sculpt_hash);
        }
        if let Some(mesh_uuid) = mesh_uuid {
            add_uuid(TileAssetType::Mesh, mesh_uuid, mesh_hash);
        }
        for (n, face) in faces.into_iter().enumerate() {
            if let Some(base_texture_uuid) = face.base_texture_uuid {
                add_uuid(TileAssetType::BaseTexture(n as u8), base_texture_uuid, Some(face.base_texture_hash));
            }
            if let Some(emissive_texture_uuid) = face.emissive_texture_uuid {
                add_uuid(TileAssetType::EmissiveTexture(n as u8), emissive_texture_uuid, face.emissive_texture_hash);
            }
        }
        Ok(results)
    }
}

fn convert_uuid(s_opt: Option<String>) -> Option<Uuid> {
    if let Some(s) = s_opt {
        match Uuid::try_parse(&s) {
            Ok(u) => Some(u),
            Err(_) => None
        }
    } else {
        None
    }
}

//  Make MySQL's type inference happy
fn convert_hash(s_opt: Option<String>) -> Option<String> {
    s_opt
}


/// The garbage collector for unused tiles.
pub struct TileGc {
    /// Which grid
    grid: String,
}

impl TileGc {
    ///  Usual new
    pub fn new(grid: &str) -> TileGc {
        Self {
            grid: grid.to_string(),
        }
    }
    
    /// Get all UUIDs in use.
    /// We have to do this the hard way, looking at each faces_json string,
    /// because this has to work with MySQL 8.0, where JSON capabiilties barely exist.
    /// In a more modern SQL implementation, we could do this whole job within SQL.
    fn get_uuids_in_use(&self, tx: &mut Transaction) -> Result<Vec<UuidUsage>, Error> {
        const SELECT_UUIDS_SQL: &str = r"SELECT 
            region_loc_x, region_loc_y, region_size_x, region_size_y,
            sculpt_uuid, sculpt_hash, mesh_uuid, mesh_hash,
            faces_json        
        FROM region_impostors
            WHERE grid = :grid;";
        let params = params!(
            "grid" => self.grid.clone(),
        );
        //  Do the select and collect up the results.
        let result: Result<Vec<Vec::<UuidUsage>>, _> = tx
        .exec_iter(SELECT_UUIDS_SQL, params)?
        .map(UuidUsage::from_row).into_iter()
        .collect();
        Ok(result?.into_iter().flatten().collect())
    }
    
    /// Build the temporary table of UUIDs in use.
    fn build_temporary_table(&self, tx: &mut Transaction) -> Result<(), Error> {
        const CREATE_INUSE_UUIDS_SQL: &str = r"CREATE TEMPORARY TABLE uuids_in_use (
            region_loc_x INT NOT NULL,
            region_loc_y INT NOT NULL,
            region_size_x INT NOT NULL,
            region_size_y INT NOT NULL,
            asset_type VARCHAR(20) NOT NULL,
            asset_uuid CHAR(36) DEFAULT NULL,
            asset_hash CHAR(8) NOT NULL)";
        const INSERT_INUSE_UUIDS_SQL: &str = r"INSERT INTO uuids_in_use
            (region_loc_x, region_loc_y, region_size_x, region_size_y, asset_type, asset_uuid, asset_hash)
            VALUES
            (:region_loc_x, :region_loc_y, :region_size_x, :region_size_y, :asset_type, :asset_uuid, :asset_hash)";
        //  Get all the active UUIDs. In memory all at once, but under 1MB
        let active_uuids = self.get_uuids_in_use(tx)?;
        log::info!("{} active UUIDs: {:?}", active_uuids.len(), &active_uuids[0..5.min(active_uuids.len())]);
         //  Create the temporary table
        tx.query_drop(CREATE_INUSE_UUIDS_SQL)?;
        log::info!("Temporary table created.");
        //  Put all the records in the temporary table.
        tx.exec_batch(
            INSERT_INUSE_UUIDS_SQL,
            active_uuids.iter().map(|p| params! {
                "region_loc_x" => p.region_loc[0],
                "region_loc_y" => p.region_loc[1],
                "region_size_x" => p.region_size[0],
                "region_size_y" => p.region_size[1],
                "asset_uuid" => p.asset_uuid.to_string(),
                "asset_hash" => p.asset_hash.clone(),
                "asset_type" => p.asset_type.to_str().to_string(),
            })
        )?;
        log::info!("Temporary table filled.");
        Ok(())
    }
    
    ///  Purge all unused tile assets
    fn purge_unused_tile_assets(&self, conn: &mut PooledConn) -> Result<(), Error> {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        self.build_temporary_table(&mut tx)?;
        const DELETE_UNUSED_TILE_ASSETS: &str = r"DELETE FROM TILE ASSETS WHERE
                ***NOT IN TEMPORARY TABLE***
        ";
        //  Create a temporary SQL table and insert all the UUIDs.
        //  This allows us to get MySQL to do the deletions.
        tx.commit()?;
        todo!();
    }
}

#[test]
/// Basic local test.
/// Requires direct but read only access to the database.
fn test_gc_locally() {
    use envie::{Envie};
    use mysql::{PooledConn, Pool};
    let _ = simplelog::CombinedLogger::init(
        vec![
            simplelog::TermLogger::new(simplelog::LevelFilter::Debug, simplelog::Config::default(), simplelog::TerminalMode::Stdout, simplelog::ColorChoice::Auto),]
    );
    
    fn purge_test(gc: &TileGc, tx: &mut Transaction) -> Result<(), Error> {
        const SELECT_UNUSED_TILE_ASSETS: &str = r"SELECT *
            FROM tile_assets t1
            WHERE NOT EXISTS (
                SELECT 1
                FROM uuids_in_use t2
                WHERE t2.region_loc_x = t1.region_loc_x
                    AND t2.region_loc_y = t1.region_loc_y
                    AND t2.region_size_x = t1.region_size_x
                    AND t2.region_size_y = t1.region_size_y
                    AND t2.asset_uuid = t1.asset_uuid
                    AND t2.asset_hash = t1.asset_hash
                    AND t1.grid = :grid
                )";
         let params = params!("grid" => gc.grid.clone());
         log::debug!("Generating deletion list");
         let result: Vec<_> = tx
        .exec_iter(SELECT_UNUSED_TILE_ASSETS, params)?
        .map(|row| log::debug!("Delete: {:?}", row))
        .collect();
        log::debug!("Generated deletion list, {} items to delete.", result.len());
        Ok(())
    }
    //  Use built-in credentials file.
    //  Not portable.
    //////const CREDSFILE: &str = "~/projects/maptools/keys/generate_credentials.txt";
    const CREDSFILE: &str = "../keys/generate_credentials.txt";
    println!("CREDSFILE: {} relative to {:?}", CREDSFILE, std::env::current_dir().unwrap());
    let credsfile = std::fs::canonicalize(CREDSFILE).expect("CREDSFILE path not valid");
    const GRID: &str = "agni";
    let creds = Envie::load_with_path(&credsfile.to_str().unwrap()).expect("Unable to open credentials file");
    let portnum =  3306;
    let opts = mysql::OptsBuilder::new()
        //  Dreamhost is still using old authentication
        .secure_auth(false)
        .ip_or_hostname(creds.get("DB_HOST"))
        .tcp_port(portnum)
        .user(creds.get("DB_USER"))
        .pass(creds.get("DB_PASS"))
        .db_name(creds.get("DB_NAME"));
    drop(creds);
    log::info!("Opts: {:?}", opts);
    let conn = Pool::new(opts).expect("Unable to create MySQL connection pool.");
    //  Have database connectionj, can try to test.
    let gc = TileGc::new(GRID);
    let mut tx = conn.start_transaction(TxOpts::default()).expect("Cannot start transaction");
    let _ = gc.build_temporary_table(&mut tx).expect("Build temporary table failed");
    purge_test(&gc, &mut tx).expect("Deletion check failed");
}
