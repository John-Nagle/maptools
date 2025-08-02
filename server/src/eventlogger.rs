use mysql::*;
use mysql::prelude::*;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::fmt;
use std::error::Error;
use chrono::Utc;
use sha1::{Sha1, Digest};
use std::sync::Arc;
use std::net::SocketAddr;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct SlVector {
    x: f32,
    y: f32,
    z: f32,
}

impl fmt::Display for SlVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SlRegion {
    name: String,
    x: i32,
    y: i32,
}

impl fmt::Display for SlRegion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({},{})", self.name, self.x, self.y)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct SlGlobalPos {
    x: f64,
    y: f64,
}

impl SlGlobalPos {
    fn set(&mut self, region: &SlRegion, pos: &SlVector) {
        self.x = region.x as f64 + pos.x as f64;
        self.y = region.y as f64 + pos.y as f64;
    }
    fn min(&mut self, other: &SlGlobalPos) {
        self.x = self.x.min(other.x);
        self.y = self.y.min(other.y);
    }
    fn max(&mut self, other: &SlGlobalPos) {
        self.x = self.x.max(other.x);
        self.y = self.y.max(other.y);
    }
    fn distance(&self, other: &SlGlobalPos) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx*dx + dy*dy).sqrt()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SlHeader {
    owner_name: String,
    shard: String,
    object_name: String,
    region: SlRegion,
    local_position: SlVector,
}

impl fmt::Display for SlHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "owner_name: \"{}\"  object_name: \"{}\"  region: {}  local_position: {}",
            self.owner_name, self.object_name, self.region, self.local_position
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VehLogEvent {
    timestamp: i64,
    serial: i32,
    tripid: String,
    severity: i8,
    eventtype: String,
    msg: String,
    auxval: f32,
    debug: i8,
}

impl fmt::Display for VehLogEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "timestamp: {}  tripid: \"{}\"  severity: {}  eventtype: {}  msg: {}  auxval: {}",
            self.timestamp, self.tripid, self.severity, self.eventtype, self.msg, self.auxval
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MysqlConfig {
    domain: String,
    database: String,
    user: String,
    password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VdbConfig {
    mysql: MysqlConfig,
    authkey: HashMap<String, String>,
}

impl fmt::Display for VdbConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keys = self.authkey.keys().cloned().collect::<Vec<_>>().join(" ");
        write!(
            f,
            "domain: {}  database: {}  user: {} authkeys: {}",
            self.mysql.domain, self.mysql.database, self.mysql.user, keys
        )
    }
}

// Parses "Vallone (462592, 306944)"
fn parse_slregion(s: &str) -> Result<SlRegion, Box<dyn Error>> {
    let ix = s.rfind('(').ok_or("SL region location not in expected format")?;
    let name = s[..ix].trim().to_string();
    let coords = s[ix..].trim_matches(|c| c == '(' || c == ')');
    let parts: Vec<&str> = coords.split(',').map(|x| x.trim()).collect();
    if parts.len() != 2 {
        return Err("Failed to parse SL region coordinates".into());
    }
    Ok(SlRegion {
        name,
        x: parts[0].parse()?,
        y: parts[1].parse()?,
    })
}

// Parses "(204.783539, 26.682831, 35.563702)"
fn parse_slvector(s: &str) -> Result<SlVector, Box<dyn Error>> {
    let s = s.trim_matches(|c| c == '(' || c == ')');
    let parts: Vec<&str> = s.split(',').map(|x| x.trim()).collect();
    if parts.len() != 3 {
        return Err("Failed to parse SL vector".into());
    }
    Ok(SlVector {
        x: parts[0].parse()?,
        y: parts[1].parse()?,
        z: parts[2].parse()?,
    })
}

fn get_header_field(headers: &HashMap<String, String>, key: &str) -> Result<String, Box<dyn Error>> {
    let v = headers.get(key)
        .ok_or(format!("HTTP header from Second Life was missing field \"{}\"", key))?
        .trim().to_string();
    if v.is_empty() {
        return Err(format!("HTTP header from Second Life was missing field \"{}\"", key).into());
    }
    Ok(v)
}

fn parse_header(headers: &HashMap<String, String>) -> Result<SlHeader, Box<dyn Error>> {
    Ok(SlHeader {
        owner_name: get_header_field(headers, "X-Secondlife-Owner-Name")?,
        object_name: get_header_field(headers, "X-Secondlife-Object-Name")?,
        shard: get_header_field(headers, "X-Secondlife-Shard")?,
        region: parse_slregion(&get_header_field(headers, "X-Secondlife-Region")?)?,
        local_position: parse_slvector(&get_header_field(headers, "X-Secondlife-Local-Position")?)?,
    })
}

fn parse_veh_event(s: &[u8]) -> Result<VehLogEvent, Box<dyn Error>> {
    let event: VehLogEvent = serde_json::from_slice(s)?;
    if event.tripid.len() != 40 {
        return Err(format!("Trip ID \"{}\" from Second Life was not 40 bytes long", event.tripid).into());
    }
    Ok(event)
}

fn hash_with_token(token: &[u8], s: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(token);
    hasher.update(s);
    hex::encode(hasher.finalize())
}

fn validate_auth_token(s: &[u8], name: &str, value: &str, config: &VdbConfig) -> Result<(), Box<dyn Error>> {
    let token = config.authkey.get(name).ok_or(format!("Logging authorization token \"{}\" not recognized.", name))?;
    let hash = hash_with_token(token.as_bytes(), s);
    if hash != value {
        return Err(format!("Logging authorization token \"{}\" failed to validate.\nText: \"{}\"\nHash sent: \"{}\"\nHash calc: \"{}\"",
            name, String::from_utf8_lossy(s), value, hash).into());
    }
    Ok(())
}

fn insert_event(conn: &mut PooledConn, hdr: &SlHeader, ev: &VehLogEvent) -> Result<(), Box<dyn Error>> {
    conn.exec_drop(r"INSERT INTO events 
        (time, shard, owner_name, object_name, region_name, region_corner_x, region_corner_y, local_position_x, local_position_y, local_position_z, tripid, severity, eventtype, msg, auxval, serial)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        (
            ev.timestamp,
            &hdr.shard,
            &hdr.owner_name,
            &hdr.object_name,
            &hdr.region.name,
            hdr.region.x,
            hdr.region.y,
            hdr.local_position.x,
            hdr.local_position.y,
            hdr.local_position.z,
            &ev.tripid,
            ev.severity,
            &ev.eventtype,
            &ev.msg,
            ev.auxval,
            ev.serial,
        ))?;
    Ok(())
}

fn insert_todo(conn: &mut PooledConn, tripid: &str) -> Result<(), Box<dyn Error>> {
    conn.exec_drop(
        r"INSERT INTO tripstodo (tripid) VALUES (?) ON DUPLICATE KEY UPDATE stamp=NOW()",
        (tripid,)
    )?;
    Ok(())
}

fn db_update(conn: &mut PooledConn, hdr: &SlHeader, ev: &VehLogEvent) -> Result<(), Box<dyn Error>> {
    let mut tx = conn.start_transaction(TxOpts::default())?;
    insert_event(&mut tx, hdr, ev)?;
    insert_todo(&mut tx, &ev.tripid)?;
    tx.commit()?;
    Ok(())
}

// The main logic to add an event
fn add_event(
    bodycontent: &[u8], 
    headers: &HashMap<String, String>, 
    config: &VdbConfig, 
    pool: &Pool,
) -> Result<(), Box<dyn Error>> {
    validate_auth_token(
        bodycontent,
        headers.get("X-Authtoken-Name").map(|s| s.trim()).unwrap_or(""),
        headers.get("X-Authtoken-Hash").map(|s| s.trim()).unwrap_or(""),
        config,
    )?;
    let hdr = parse_header(headers)?;
    let ev = parse_veh_event(bodycontent)?;
    let mut conn = pool.get_conn()?;
    db_update(&mut conn, &hdr, &ev)?;
    Ok(())
}

// You would use warp/axum/actix-web for HTTP handling in Rust. Handler stub:
async fn handle_request(
    body: Vec<u8>,
    headers: HashMap<String, String>,
    config: Arc<VdbConfig>,
    pool: Pool,
) -> Result<(), String> {
    match add_event(&body, &headers, &config, &pool) {
        Ok(_) => Ok("Event added".to_string()),
        Err(e) => Err(format!("Internal server error: {}", e)),
    }
}

// Reading config from JSON file
fn read_config(config_path: &str) -> Result<VdbConfig, Box<dyn Error>> {
    let path = shellexpand::tilde(config_path).to_string();
    let data = fs::read_to_string(&path)?;
    let config: VdbConfig = serde_json::from_str(&data)?;
    Ok(config)
}
