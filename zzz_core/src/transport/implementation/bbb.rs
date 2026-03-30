use actix_web::HttpResponse;
//zzz_core/src/transport/implementation/bbb.rs
use async_trait::async_trait;
use crate::binary_data_process::z_base::{Base64, Encoder as _};
use crate::transport::base::{TransportHttpB, TransportHttpType, TransportTrait};
use once_cell::sync::Lazy;
use regex::Regex;


const HTML: &'static str = include_str!("./百度文心助手 - 办公学习一站解决.payload.html");
const PNG_BASE64_PREFIX: &'static str = "iVBORw0KGgoAAAANSUhEUgAABDMAAAUbCAYAAA";
static RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"display: none;background: url\("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAABDMAAAUbCAYAAA([^"\s]+)"\)"#)
        .unwrap()
});


pub struct BBB;
#[async_trait]
impl TransportTrait for BBB {
    const SUPPORT: &'static str = "zy:yz";
    const PROCESS: &'static str = "b";
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
            let html = format!(
                "{}\n{}",
                HTML,
                format!(r#"<style>body script {{display: none;background: url("data:image/png;base64,{}{}");}}</style>"#,
                PNG_BASE64_PREFIX, Base64::encode(bytes))
            );
            return Some(TransportHttpType::HttpResponse(
                HttpResponse::Ok()
                    .content_type("text/html; charset=utf-8")
                    .body(html),
            ));
        }
        None
    }
}

#[async_trait]
impl TransportHttpB for BBB {
}

#[cfg(test)]
mod tests {
    use actix_web::rt::Runtime;
    use super::*;
    use std::thread::{self, JoinHandle};
    use std::sync::mpsc;
    use std::net::SocketAddr;
    use tokio::runtime::Runtime as OtherRuntime;
    use actix_web::{App, HttpServer, web, get, Responder};
    use reqwest::blocking::Client;


    const TEST_DATA: &[u8; 36] = b"f8d1763a-a376-40e9-9b1e-40e5e917d96d";

    #[get("/greet")]
    async fn greet() -> impl Responder {
        if let Some(TransportHttpType::HttpResponse(resp)) = BBB::inject(
            TransportHttpType::Payload(Vec::from(TEST_DATA))
        ) {
            return resp;
        }
        HttpResponse::Ok().body(HTML)
    }

    pub struct WebServer {
        join_handle: Option<JoinHandle<()>>,
    }
    impl WebServer {
        pub fn new() -> Self {
            Self {join_handle: None}
        }
        pub fn start(&mut self) -> Result<u16, Box<dyn std::error::Error>> {
            if self.join_handle.is_some() {
                return Err("Server is already running".into());
            }
            let (tx, rx) = mpsc::channel::<u16>();
            let (err_tx, err_rx) = mpsc::channel::<String>();

            let handle = thread::spawn(move || {
                let rt = match Runtime::new() {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = err_tx.send(format!("Failed to create Tokio runtime: {}", e));
                        return;
                    }
                };

                rt.block_on(async move {
                    let server = HttpServer::new(move || {
                        App::new()
                            .service(greet)
                    })
                        .bind(format!("{}:0", "0.0.0.0")); // 绑定到 0 端口以获取随机可用端口

                    let server = match server {
                        Ok(s) => s,
                        Err(e) => {
                            let _ = err_tx.send(format!("Failed to bind to {}: {}", "0.0.0.0", e));
                            return;
                        }
                    };

                    if let Some(addr) = server.addrs().first() {
                        let _ = tx.send(addr.port());
                    }

                    if let Err(e) = server.run().await {
                        eprintln!("Web server runtime error: {}", e);
                    }
                });
            });

            loop {
                if let Ok(port) = rx.try_recv() {
                    self.join_handle = Some(handle);
                    return Ok(port);
                }
                if let Ok(err_msg) = err_rx.try_recv() {
                    return Err(err_msg.into());
                }
                if handle.is_finished() {
                    return Err("Thread terminated unexpectedly during startup".into());
                }
                thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        pub fn join(&mut self) {
            if let Some(handle) = self.join_handle.take() {
                let _ = handle.join();
            }
        }

        pub fn stop(&mut self) {
        }
    }

    #[test]
    fn test() {
        let mut web = WebServer::new();
        match web.start() {
            Ok(port) => {
                let url = format!("http://127.0.0.1:{}/greet", port);
                let builder = Client::new().get(&url);
                let data = BBB::extract(TransportHttpType::RequestBuilder(builder));
                assert!(data.is_some());
                assert_eq!(data, Some(Vec::from(TEST_DATA)));


            }
            Err(e) => {
                eprintln!("Failed to start server: {}", e);
            }
        }
    }
}