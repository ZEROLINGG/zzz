use crate::transport::base::{TransportHttpB, TransportHttpType, TransportTrait};
use actix_web::HttpResponse;
use image::{
    ImageEncoder, Rgba, RgbaImage,
    codecs::png::{CompressionType, FilterType, PngEncoder},
};
use once_cell::sync::Lazy;
use std::io::Cursor;


macro_rules! define_carrier {
    ($raw:ident, $rgba:ident, $file:expr) => {
        static $raw: Lazy<Vec<u8>> = Lazy::new(|| include_bytes!($file).to_vec());
        static $rgba: Lazy<Option<RgbaImage>> = Lazy::new(|| {
            image::load_from_memory(&$raw)
                .ok()
                .map(|img| img.into_rgba8())
        });
    };
}

define_carrier!(CARRIER_PNG_RAW_1, CARRIER_RGBA_1, "png_lsb_xor.1.png");
define_carrier!(CARRIER_PNG_RAW_2, CARRIER_RGBA_2, "png_lsb_xor.2.png");


macro_rules! define_lsb_xor_image {
    ($name:ident, $max_payload:expr, $carrier_rgba:expr, $carrier_raw:expr) => {
        pub struct $name;

        impl TransportTrait for $name {
            const SUPPORT: &'static str = "zy:yz";
            const PROCESS: &'static str = "b";
            const MAX_PAYLOAD_SIZE: usize = $max_payload;

            type ExtractIn  = TransportHttpType;
            type InjectIn   = TransportHttpType;
            type InjectOut  = TransportHttpType;

            fn extract(input: Self::ExtractIn) -> Option<Vec<u8>> {
                do_extract(input, &$carrier_rgba, Self::MAX_PAYLOAD_SIZE)
            }

            fn inject(input: Self::InjectIn) -> Option<Self::InjectOut> {
                do_inject(input, &$carrier_rgba, &$carrier_raw)
            }
        }

        impl TransportHttpB for $name {}
    };
}

define_lsb_xor_image!(LsbXorImage1, 6300, CARRIER_RGBA_1, CARRIER_PNG_RAW_1);
define_lsb_xor_image!(LsbXorImage2, 3000, CARRIER_RGBA_2, CARRIER_PNG_RAW_2);


fn do_extract(
    input: TransportHttpType,
    carrier: &Lazy<Option<RgbaImage>>,
    max_payload_size: usize,
) -> Option<Vec<u8>> {
    if let TransportHttpType::RequestBuilder(builder) = input {
        let bytes = builder.send().ok()?.bytes().ok()?;
        let img_org = carrier.as_ref()?;
        let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
        Some(lsb_xor_extract(&img, img_org, max_payload_size))
    } else {
        None
    }
}

fn do_inject(
    input: TransportHttpType,
    carrier: &Lazy<Option<RgbaImage>>,
    carrier_raw: &Lazy<Vec<u8>>,
) -> Option<TransportHttpType> {
    if let TransportHttpType::Payload(payload) = input {
        let img_org = carrier.as_ref()?;
        let mut img = img_org.clone();
        lsb_xor_inject(&mut img, img_org, &payload);

        let mut buffer = Vec::with_capacity(carrier_raw.len());
        PngEncoder::new_with_quality(
            Cursor::new(&mut buffer),
            CompressionType::Fast,
            FilterType::Sub,
        )
            .write_image(
                img.as_raw(),
                img.width(),
                img.height(),
                image::ExtendedColorType::Rgba8,
            )
            .ok()?;

        Some(TransportHttpType::HttpResponse(
            HttpResponse::Ok().content_type("image/png").body(buffer),
        ))
    } else {
        None
    }
}


fn lsb_xor_inject(img: &mut RgbaImage, img_org: &RgbaImage, payload: &[u8]) {
    let len_bytes = (payload.len() as u32).to_le_bytes();
    let available = (img.width() as usize) * (img.height() as usize);
    if 4 + payload.len() > available {
        return;
    }
    let mut pixels = img.pixels_mut().zip(img_org.pixels());
    for byte in len_bytes.iter().chain(payload.iter()) {
        if let Some((px, px_org)) = pixels.next() {
            inject_byte_into_pixel(px, px_org, *byte);
        } else {
            break;
        }
    }
}

fn lsb_xor_extract(
    img: &RgbaImage,
    img_org: &RgbaImage,
    max_payload_size: usize,
) -> Vec<u8> {
    if img.dimensions() != img_org.dimensions() {
        return Vec::new();
    }
    let mut pixels = img.pixels().zip(img_org.pixels());

    let mut len_buf = [0u8; 4];
    for b in &mut len_buf {
        match pixels.next() {
            Some((p, po)) => *b = extract_byte_from_pixel(p, po),
            None => return Vec::new(),
        }
    }

    let payload_len = u32::from_le_bytes(len_buf) as usize;
    if payload_len == 0 || payload_len > max_payload_size {
        return Vec::new();
    }
    let available = (img.width() as usize) * (img.height() as usize);
    if 4 + payload_len > available {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(payload_len);
    for _ in 0..payload_len {
        match pixels.next() {
            Some((p, po)) => result.push(extract_byte_from_pixel(p, po)),
            None => return Vec::new(),
        }
    }
    result
}

#[inline(always)]
fn inject_byte_into_pixel(pixel: &mut Rgba<u8>, pixel_org: &Rgba<u8>, byte: u8) {
    for ch in 0..4 {
        let orig_low2 = pixel_org[ch] & 0x03;
        let data_bits = (byte >> (ch * 2)) & 0x03;
        pixel[ch] = (pixel[ch] & 0xFC) | (data_bits ^ orig_low2);
    }
}

#[inline(always)]
fn extract_byte_from_pixel(pixel: &Rgba<u8>, pixel_org: &Rgba<u8>) -> u8 {
    let mut byte = 0u8;
    for ch in 0..4 {
        byte |= ((pixel[ch] ^ pixel_org[ch]) & 0x03) << (ch * 2);
    }
    byte
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::base::WebServer;
    use crate::web_register;
    use reqwest::blocking::Client;

    type HTTP = TransportHttpType;

    const TEST_DATA_1: &[u8] =
        b"f8d1763a-a376-40e9-9b1f8d1763a-a376-40e9-9b1e-40e5e0e5e917d96d";
    const TEST_DATA_2: &[u8] = b"caec54d4357d07c0aa14a9b52ea8b0604b789ddbcaec54d4357d07c0aa14a9b52ea8b0604b789ddb";


    async fn serve_lsb_1() -> HttpResponse {
        if let Some(HTTP::HttpResponse(resp)) =
            LsbXorImage1::inject(HTTP::Payload(Vec::from(TEST_DATA_1)))
        {
            resp
        } else {
            HttpResponse::InternalServerError().finish()
        }
    }

    async fn serve_lsb_2() -> HttpResponse {
        if let Some(HTTP::HttpResponse(resp)) =
            LsbXorImage2::inject(HTTP::Payload(Vec::from(TEST_DATA_2)))
        {
            resp
        } else {
            HttpResponse::InternalServerError().finish()
        }
    }


    #[test]
    fn test_lsb_image1_roundtrip() {
        let mut server = WebServer::new(0);
        web_register!(server {
            get "/static/image/8788b4072e19f4f2b7723e361f41b6e1481818f6.png" => serve_lsb_1,
        });
        let port = server.start().unwrap();
        let url = format!("http://127.0.0.1:{}/static/image/8788b4072e19f4f2b7723e361f41b6e1481818f6.png", port);

        let extracted =
            LsbXorImage1::extract(HTTP::RequestBuilder(Client::new().get(&url)));

        assert!(extracted.is_some(), "Image1: payload 未能提取");
        assert_eq!(extracted.unwrap(), TEST_DATA_1, "Image1: 数据不一致");
        server.stop();
    }
    #[test]
    fn test_lsb_image2_roundtrip() {
        let mut server = WebServer::new(0);
        web_register!(server {
            get "/static/image/caec54d4357d07c0aa14a9b52ea8b0604b789ddb.png" => serve_lsb_2,
        });
        let port = server.start().unwrap();
        let url = format!("http://127.0.0.1:{}/static/image/caec54d4357d07c0aa14a9b52ea8b0604b789ddb.png", port);

        let extracted =
            LsbXorImage2::extract(HTTP::RequestBuilder(Client::new().get(&url)));

        assert!(extracted.is_some(), "Image2: payload 未能提取");
        assert_eq!(extracted.unwrap(), TEST_DATA_2, "Image2: 数据不一致");
        server.stop();
    }
}