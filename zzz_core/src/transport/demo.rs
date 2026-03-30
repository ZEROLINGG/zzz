// src/transport/demo.rs
use actix_web::{web::Bytes, HttpRequest, HttpResponse};
use reqwest::{RequestBuilder, Response};
use crate::transport::base::{Client, Transport, Web};
use crate::binary_data_process::z_base::{Base64, Encoder as _};


pub struct Hello;

const HTML: &'static str = include_str!("github_page.html");

const TARGET: &'static str = r#"snippet-clipboard-copy-button-unpositioned"#;

const PNG_BASE64_PREFIX: &'static str = "iVBORw0KGgoAAAANSUhEUgAABDMAAAUbCAYAAA";

pub fn insert_hidden_payload(html: &str, target: &str, base64_payload: &str) -> String {
    let insert = format!(
        r#"<span class="hidden" style="display:none !important; visibility:hidden; position:absolute;">
    <span src="data:image/png;base64,{}{}"></span>
</span>"#,
        PNG_BASE64_PREFIX, base64_payload
    );

    if let Some(pos) = html.find(target) {
        let insert_pos = html[pos..]
            .find('\n')
            .map(|i| pos + i + 1)
            .unwrap_or(html.len());

        let mut result = String::with_capacity(html.len() + insert.len() + 64);
        result.push_str(&html[..insert_pos]);
        result.push_str(&insert);
        result.push('\n');
        result.push_str(&html[insert_pos..]);
        result
    } else {
        let mut result = html.to_string();
        result.push_str(&insert);
        result
    }
}

pub fn extract_hidden_payload(html: &str, target: &str) -> Option<String> {
    let pos = html.find(target)?;
    let search_start = pos + target.len();

    let base64_start = html[search_start..].find("base64,")?;
    let start_idx = search_start + base64_start + 7;

    let remaining = &html[start_idx..];
    let end_idx = remaining
        .find('"')
        .or_else(|| remaining.find('\''))
        .map(|i| start_idx + i)
        .unwrap_or(html.len());

    let mut base64_data = &html[start_idx..end_idx];

    if base64_data.starts_with(PNG_BASE64_PREFIX) {
        base64_data = &base64_data[PNG_BASE64_PREFIX.len()..];
    }

    Some(base64_data.trim().to_string())
}

impl Web for Hello {
    fn get_payload_w(req: HttpRequest, body: Bytes) -> Option<Vec<u8>> {
        let content_type = req
            .headers()
            .get("content-type")?
            .to_str()
            .ok()?;

        if !content_type.contains("application/x-www-form-urlencoded") {
            return None;
        }

        let body_str = std::str::from_utf8(&body).ok()?;

        for pair in body_str.split('&') {
            let mut kv = pair.splitn(2, '=');
            let key = kv.next()?;
            let value = kv.next()?;

            if key == "comment" {
                let decoded = urlencoding::decode(value).ok()?;
                return Base64::decode(decoded.as_ref());
            }
        }
        None
    }

    fn set_payload_w<T: AsRef<[u8]>>(payload: T) -> Option<HttpResponse> {
        let payload_base64 = Base64::encode(payload);
        let html = insert_hidden_payload(HTML, TARGET, &payload_base64);
        Some(
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(html),
        )
    }
}

impl Client for Hello {
    async fn get_payload_c(rep: Response) -> Option<Vec<u8>> {
        let html = rep.text().await.ok()?;

        let base64_str = extract_hidden_payload(&html, TARGET)?;

        Base64::decode(&base64_str)
    }

    fn set_payload_c<T: AsRef<[u8]>>(
        payload: T,
        url_base: &str,
        url_path: &str,
    ) -> Option<RequestBuilder> {
        let client = reqwest::Client::new();
        let url = format!("{}{}", url_base, url_path);

        let payload_base64 = Base64::encode(payload);

        let form = [
            ("username", "guest"),
            ("comment", payload_base64.as_str()),
        ];

        Some(client.post(url).form(&form))
    }
}

impl Transport for Hello {
    const URL_PATH: &'static str = "/index.html";
    // const IS_REVERSE: bool = false;
}

#[actix_web::test]
async fn test_hello() {
    use actix_web::{test, web, App, HttpResponse, http::header};

    let original_payload = b"hello-stego-payload-123456";

    let app = test::init_service(
        App::new().route(
            Hello::URL_PATH,
            web::post().to(move |req: HttpRequest, body: Bytes| async move {
                let extracted = Hello::get_payload_w(req, body)
                    .expect("server failed to extract payload");

                assert_eq!(extracted, original_payload);

                Hello::set_payload_w(&extracted)
                    .unwrap_or_else(|| HttpResponse::InternalServerError().finish())
            }),
        ),
    )
        .await;

    let builder = Hello::set_payload_c(
        original_payload,
        "http://localhost",
        Hello::URL_PATH,
    )
        .expect("failed to build request");

    let request = builder.build().expect("failed to build reqwest request");

    let mut actix_req = test::TestRequest::post()
        .uri(request.url().path())
        .insert_header((
            header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        ));

    if let Some(body) = request.body() {
        if let Some(bytes) = body.as_bytes() {
            actix_req = actix_req.set_payload(bytes.to_vec());
        }
    }

    let actix_req = actix_req.to_request();

    let resp = test::call_service(&app, actix_req).await;

    assert!(resp.status().is_success());

    let body_bytes = test::read_body(resp).await;
    let html = String::from_utf8(body_bytes.to_vec()).expect("invalid utf8");

    let extracted_base64 =
        extract_hidden_payload(&html, TARGET).expect("failed to extract base64");

    let final_payload =
        Base64::decode(&extracted_base64).expect("failed to decode base64");

    assert_eq!(final_payload, original_payload);
}