
use sha1::{Sha1, Digest as Sha1Digest};
use sha2::{Sha256, Digest as Sha2Digest};
use chrono::Local;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::uuid;

pub fn uuid() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "").to_uppercase()
}




pub fn sha1(s: &[u8]) -> String {
    hex::encode(<Sha1 as Sha1Digest>::digest(s))
}

pub fn sha256(s: &[u8]) -> String {
    hex::encode(<Sha256 as Sha2Digest>::digest(s))
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

