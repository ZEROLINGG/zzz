use rand::thread_rng;
use rand::distributions::{Alphanumeric, DistString};  // rand 0.8: distributions 模块，DistString trait
use sha1::{Sha1, Digest as Sha1Digest};
use sha2::{Sha256, Digest as Sha2Digest};
use base64::{engine::general_purpose, Engine as _};
use chrono::Local;
use std::time::{SystemTime, UNIX_EPOCH};


pub fn uuid() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "").to_uppercase()
}




pub fn sha1(s: &[u8]) -> String {
    hex::encode(<Sha1 as Sha1Digest>::digest(s))
}

pub fn sha256(s: &[u8]) -> String {
    hex::encode(<Sha256 as Sha2Digest>::digest(s))
}


pub fn b64encode(s: &[u8]) -> String {
    general_purpose::STANDARD.encode(s)
}

pub fn b64decode(s: &str) -> anyhow::Result<Vec<u8>> {
    Ok(general_purpose::STANDARD.decode(s)?)
}

pub fn timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64
}

pub fn format_timestamp() -> String {
    Local::now().format("%Y-%m-%d-%H-%M-%S").to_string()
}

