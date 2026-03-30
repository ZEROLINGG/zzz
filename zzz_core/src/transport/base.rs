//src/transport/base.rs
use actix_web::{web, HttpRequest, HttpResponse};
use reqwest::{RequestBuilder,Response};



pub trait Web {
    fn get_payload_w(req: HttpRequest, body: web::Bytes) -> Option<Vec<u8>>;
    fn set_payload_w<T: AsRef<[u8]>>(payload: T) -> Option<HttpResponse>;
}

pub trait Client {
    async fn get_payload_c(rep: Response) -> Option<Vec<u8>>;
    fn set_payload_c<T: AsRef<[u8]>>(payload: T,url_base: &str,url_path: &str) -> Option<RequestBuilder>;
}


pub trait Transport: Web + Client {
    const URL_PATH: &'static str;

    // const IS_REVERSE: bool;     // 此处定义从受感染主机到c2服务器为正向

}


