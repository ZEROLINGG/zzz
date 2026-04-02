//zzz_core/src/transport/implementation/fake_base64_image.rs
#![allow(dead_code)]
use std::fmt::Write;
use crate::binary_data_process::z_base::{Base64, Encoder as _};
use crate::transport::base::{TransportHttpB, TransportHttpType, TransportTrait};
use actix_web::HttpResponse;
use obfstr::obfstr;
use once_cell::sync::Lazy;
use regex::Regex;

static HTML: Lazy<String> = Lazy::new(|| {
    format!("{}",obfstr!(include_str!("./fake_base64_image.1.html")))
});

static PNG_BASE64_PREFIX: Lazy<String> = Lazy::new(|| {
    format!("{}",obfstr!("iVBORw0KGgoAAAANSUhEUgAABDMAAAUbCAYAAA"))
});static RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(obfstr!(
        r#"display: none;background: url\("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAABDMAAAUbCAYAAA([^"\s]+)"\)"#
    )).unwrap()
});

pub struct FakeBase64Image;
impl TransportTrait for FakeBase64Image {
    const SUPPORT: &'static str = "zy:yz";
    const PROCESS: &'static str = "b";
    const MAX_PAYLOAD_SIZE: usize = 128 * 1024;
    type ExtractIn = TransportHttpType;
    type InjectIn = TransportHttpType;
    type InjectOut = TransportHttpType;

    fn extract(input: Self::ExtractIn) -> Option<Vec<u8>> {
        if let TransportHttpType::RequestBuilder(builder) = input {
            let response = builder.send().ok()?;
            let text = response.text().ok()?;
            let caps = RE.captures(&text)?;
            let encoded_part = caps.get(1)?.as_str();
            let decoded = Base64::decode(encoded_part)?;
            return Some(decoded);
        }

        None
    }

    fn inject(input: Self::InjectIn) -> Option<Self::InjectOut> {
        if let TransportHttpType::Payload(bytes) = input {
            let mut html = HTML.clone();
            write!(
                &mut html,
                "{}{}{}",
                obfstr!("\n<style>body script {{display: none;background: url(\"data:image/png;base64,"),
                *PNG_BASE64_PREFIX,
                Base64::encode(bytes),
            ).unwrap();
            write!(
                &mut html,
                "{}",
                obfstr!("\");}}</style>")
            ).unwrap();
            return Some(TransportHttpType::HttpResponse(
                HttpResponse::Ok()
                    .content_type("text/html; charset=utf-8")
                    .body(html),
            ));
        }
        None
    }
}

impl TransportHttpB for FakeBase64Image {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::base::{WebServer};
    use reqwest::blocking::Client;
    use crate::web_register;

    type HTTP = TransportHttpType;

    const TEST_DATA: &[u8; 62] = b"f8d1763a-a376-40e9-9b1f8d1763a-a376-40e9-9b1e-40e5e0e5e917d96d";

    async fn greet() -> HttpResponse {
        if let Some(HTTP::HttpResponse(resp)) =
            FakeBase64Image::inject(HTTP::Payload(Vec::from(TEST_DATA)))
        {
            return resp;
        }
        HttpResponse::Ok().body(&**HTML)
    }

    #[test]
    fn test() {
        let mut server = WebServer::new(0);

        web_register!(server {
            get "/greet" => greet,
        });

        let port = server.start().unwrap();

        let url = format!("http://127.0.0.1:{}/greet", port);
        let builder = Client::new().get(&url);

        let data = FakeBase64Image::extract(TransportHttpType::RequestBuilder(builder));

        assert!(data.is_some());
        assert_eq!(data, Some(Vec::from(TEST_DATA)));

        println!("{:?}", Some(Vec::from(TEST_DATA)));
        println!("{:?}", data);

        server.stop();
    }
}
