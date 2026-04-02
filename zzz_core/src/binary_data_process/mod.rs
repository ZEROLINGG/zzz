//zzz_core/src/binary_data_process/mod.rs

pub mod z_rsa;
pub mod z_aes;
pub mod z_base;
pub mod z_compress;
pub mod pack;
pub mod z_bit;

use z_aes::{Cipher, Aes128Gcm, Aes256Gcm, Aes128Ctr, Aes256Ctr};
use z_base::{Encoder, Base64, Base85, Base91};
use z_compress::{Compressor, Lz4, Gzip, Zstd};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum COMPRESS {
    Lz4 = 11,
    Gzip = 12,
    Zstd = 13,
    None = 10,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AES {
    Aes128Gcm = 21,
    Aes256Gcm = 22,
    Aes128Ctr = 23,
    Aes256Ctr = 24,
    None = 20,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BASE {
    Base64 = 31,
    Base85 = 32,
    Base91 = 33,
}

impl COMPRESS {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            11 => Some(COMPRESS::Lz4),
            12 => Some(COMPRESS::Gzip),
            13 => Some(COMPRESS::Zstd),
            10 => Some(COMPRESS::None),
            _ => None,
        }
    }
}

impl AES {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            21 => Some(AES::Aes128Gcm),
            22 => Some(AES::Aes256Gcm),
            23 => Some(AES::Aes128Ctr),
            24 => Some(AES::Aes256Ctr),
            20 => Some(AES::None),
            _ => None,
        }
    }
}

impl BASE {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            31 => Some(BASE::Base64),
            32 => Some(BASE::Base85),
            33 => Some(BASE::Base91),
            _ => None,
        }
    }
}

fn apply_compress(input: &[u8], kind: COMPRESS) -> Vec<u8> {
    let mut result = match kind {
        COMPRESS::Lz4 => Lz4::compress(input),
        COMPRESS::Gzip => Gzip::compress(input),
        COMPRESS::Zstd => Zstd::compress(input),
        COMPRESS::None => input.to_vec(),
    };
    // 添加魔术前缀（字节）
    let mut output = vec![kind as u8];
    output.append(&mut result);
    output
}

fn apply_decompress(input: &[u8]) -> Option<Vec<u8>> {
    if input.is_empty() {
        return None;
    }

    let kind = COMPRESS::from_u8(input[0])?;
    let data = &input[1..];

    match kind {
        COMPRESS::Lz4 => Lz4::decompress(data),
        COMPRESS::Gzip => Gzip::decompress(data),
        COMPRESS::Zstd => Zstd::decompress(data),
        COMPRESS::None => Some(data.to_vec()),
    }
}

fn apply_encrypt(input: &[u8], kind: AES, key: &[u8]) -> Option<Vec<u8>> {
    let mut result = match kind {
        AES::Aes128Gcm => Aes128Gcm::encrypt(key.try_into().ok()?, input)?,
        AES::Aes256Gcm => Aes256Gcm::encrypt(key.try_into().ok()?, input)?,
        AES::Aes128Ctr => Aes128Ctr::encrypt(key.try_into().ok()?, input)?,
        AES::Aes256Ctr => Aes256Ctr::encrypt(key.try_into().ok()?, input)?,
        AES::None => input.to_vec(),
    };
    let mut output = vec![kind as u8];
    output.append(&mut result);
    Some(output)
}

fn apply_decrypt(input: &[u8], key: &[u8]) -> Option<Vec<u8>> {
    if input.is_empty() {
        return None;
    }
    let kind = AES::from_u8(input[0])?;
    let data = &input[1..];
    match kind {
        AES::Aes128Gcm => Aes128Gcm::decrypt(key.try_into().ok()?, data),
        AES::Aes256Gcm => Aes256Gcm::decrypt(key.try_into().ok()?, data),
        AES::Aes128Ctr => Aes128Ctr::decrypt(key.try_into().ok()?, data),
        AES::Aes256Ctr => Aes256Ctr::decrypt(key.try_into().ok()?, data),
        AES::None => Some(data.to_vec()),
    }
}

fn apply_encode(input: &[u8], kind: BASE) -> String {
    let encoded = match kind {
        BASE::Base64 => Base64::encode(input),
        BASE::Base85 => Base85::encode(input),
        BASE::Base91 => Base91::encode(input),
    };
    // 添加魔术前缀（字符）
    let prefix = (kind as u8) as char;
    format!("{}{}", prefix, encoded)
}

fn apply_decode(input: &str) -> Option<Vec<u8>> {
    if input.is_empty() {
        return None;
    }

    let prefix_char = input.chars().next()?;
    let kind = BASE::from_u8(prefix_char as u8)?;
    let data = &input[1..];

    match kind {
        BASE::Base64 => Base64::decode(data),
        BASE::Base85 => Base85::decode(data),
        BASE::Base91 => Base91::decode(data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── COMPRESS ──────────────────────────────────────────────────────────────

    #[test]
    fn compress_from_u8_known_values() {
        assert_eq!(COMPRESS::from_u8(10), Some(COMPRESS::None));
        assert_eq!(COMPRESS::from_u8(11), Some(COMPRESS::Lz4));
        assert_eq!(COMPRESS::from_u8(12), Some(COMPRESS::Gzip));
        assert_eq!(COMPRESS::from_u8(13), Some(COMPRESS::Zstd));
    }

    #[test]
    fn compress_from_u8_unknown_returns_none() {
        assert_eq!(COMPRESS::from_u8(0), None);
        assert_eq!(COMPRESS::from_u8(14), None);
        assert_eq!(COMPRESS::from_u8(255), None);
    }

    #[test]
    fn apply_compress_prepends_magic_byte() {
        let data = b"hello world";
        let out = apply_compress(data, COMPRESS::None);
        assert_eq!(out[0], COMPRESS::None as u8);
        assert_eq!(&out[1..], data);
    }

    #[test]
    fn roundtrip_compress_none() {
        let data = b"roundtrip none";
        let compressed = apply_compress(data, COMPRESS::None);
        let decompressed = apply_decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn roundtrip_compress_lz4() {
        let data = b"lz4 lz4 lz4 lz4 lz4 lz4 lz4 lz4 lz4 lz4";
        let compressed = apply_compress(data, COMPRESS::Lz4);
        assert_eq!(compressed[0], COMPRESS::Lz4 as u8);
        let decompressed = apply_decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn roundtrip_compress_gzip() {
        let data = b"gzip test data with some repetition repetition repetition";
        let compressed = apply_compress(data, COMPRESS::Gzip);
        assert_eq!(compressed[0], COMPRESS::Gzip as u8);
        let decompressed = apply_decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn roundtrip_compress_zstd() {
        let data = b"zstd test data with some repetition repetition repetition";
        let compressed = apply_compress(data, COMPRESS::Zstd);
        assert_eq!(compressed[0], COMPRESS::Zstd as u8);
        let decompressed = apply_decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn decompress_empty_input_returns_none() {
        assert_eq!(apply_decompress(&[]), None);
    }

    #[test]
    fn decompress_invalid_magic_byte_returns_none() {
        let bad = vec![0xFFu8, 1, 2, 3];
        assert_eq!(apply_decompress(&bad), None);
    }

    #[test]
    fn roundtrip_compress_empty_payload() {
        let data: &[u8] = b"";
        let compressed = apply_compress(data, COMPRESS::Lz4);
        let decompressed = apply_decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    // ── AES ───────────────────────────────────────────────────────────────────

    #[test]
    fn aes_from_u8_known_values() {
        assert_eq!(AES::from_u8(20), Some(AES::None));
        assert_eq!(AES::from_u8(21), Some(AES::Aes128Gcm));
        assert_eq!(AES::from_u8(22), Some(AES::Aes256Gcm));
        assert_eq!(AES::from_u8(23), Some(AES::Aes128Ctr));
        assert_eq!(AES::from_u8(24), Some(AES::Aes256Ctr));
    }

    #[test]
    fn aes_from_u8_unknown_returns_none() {
        assert_eq!(AES::from_u8(0), None);
        assert_eq!(AES::from_u8(25), None);
        assert_eq!(AES::from_u8(255), None);
    }

    #[test]
    fn apply_encrypt_none_prepends_magic_and_roundtrips() {
        let data = b"plaintext data";
        let encrypted = apply_encrypt(data, AES::None, b"").unwrap();
        assert_eq!(encrypted[0], AES::None as u8);
        let decrypted = apply_decrypt(&encrypted, b"").unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn roundtrip_aes128gcm() {
        let key = b"0123456789abcdef"; // 16 bytes
        let data = b"secret message for aes-128-gcm";
        let encrypted = apply_encrypt(data, AES::Aes128Gcm, key).unwrap();
        assert_eq!(encrypted[0], AES::Aes128Gcm as u8);
        let decrypted = apply_decrypt(&encrypted, key).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn roundtrip_aes256gcm() {
        let key = b"0123456789abcdef0123456789abcdef"; // 32 bytes
        let data = b"secret message for aes-256-gcm";
        let encrypted = apply_encrypt(data, AES::Aes256Gcm, key).unwrap();
        assert_eq!(encrypted[0], AES::Aes256Gcm as u8);
        let decrypted = apply_decrypt(&encrypted, key).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn roundtrip_aes128ctr() {
        let key = b"0123456789abcdef"; // 16 bytes
        let data = b"secret message for aes-128-ctr";
        let encrypted = apply_encrypt(data, AES::Aes128Ctr, key).unwrap();
        assert_eq!(encrypted[0], AES::Aes128Ctr as u8);
        let decrypted = apply_decrypt(&encrypted, key).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn roundtrip_aes256ctr() {
        let key = b"0123456789abcdef0123456789abcdef"; // 32 bytes
        let data = b"secret message for aes-256-ctr";
        let encrypted = apply_encrypt(data, AES::Aes256Ctr, key).unwrap();
        assert_eq!(encrypted[0], AES::Aes256Ctr as u8);
        let decrypted = apply_decrypt(&encrypted, key).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn encrypt_without_key_returns_none_for_real_cipher() {
        let data = b"no key provided";
        assert_eq!(apply_encrypt(data, AES::Aes128Gcm, b""), None);
        assert_eq!(apply_encrypt(data, AES::Aes256Gcm, b""), None);
        assert_eq!(apply_encrypt(data, AES::Aes128Ctr, b""), None);
        assert_eq!(apply_encrypt(data, AES::Aes256Ctr, b""), None);
    }

    #[test]
    fn decrypt_empty_input_returns_none() {
        assert_eq!(apply_decrypt(&[], b""), None);
    }

    #[test]
    fn decrypt_invalid_magic_byte_returns_none() {
        let bad = vec![0xFFu8, 1, 2, 3];
        assert_eq!(apply_decrypt(&bad, b""), None);
    }

    #[test]
    fn decrypt_wrong_key_returns_none_or_garbage_for_gcm() {
        // GCM has authentication — wrong key must return None
        let key = b"0123456789abcdef";
        let wrong_key = b"fedcba9876543210";
        let data = b"authenticated payload";
        let encrypted = apply_encrypt(data, AES::Aes128Gcm, key).unwrap();
        assert_eq!(apply_decrypt(&encrypted, wrong_key), None);
    }

    #[test]
    fn roundtrip_encrypt_empty_payload() {
        let key = b"0123456789abcdef0123456789abcdef";
        let data: &[u8] = b"";
        let encrypted = apply_encrypt(data, AES::Aes256Gcm, key).unwrap();
        let decrypted = apply_decrypt(&encrypted, key).unwrap();
        assert_eq!(decrypted, data);
    }

    // ── BASE ──────────────────────────────────────────────────────────────────

    #[test]
    fn base_from_u8_known_values() {
        assert_eq!(BASE::from_u8(31), Some(BASE::Base64));
        assert_eq!(BASE::from_u8(32), Some(BASE::Base85));
        assert_eq!(BASE::from_u8(33), Some(BASE::Base91));
    }

    #[test]
    fn base_from_u8_unknown_returns_none() {
        assert_eq!(BASE::from_u8(0), None);
        assert_eq!(BASE::from_u8(30), None);
        assert_eq!(BASE::from_u8(34), None);
        assert_eq!(BASE::from_u8(255), None);
    }

    #[test]
    fn apply_encode_prepends_magic_char() {
        let data = b"test";
        for (kind, expected_prefix) in [
            (BASE::Base64, BASE::Base64 as u8 as char),
            (BASE::Base85, BASE::Base85 as u8 as char),
            (BASE::Base91, BASE::Base91 as u8 as char),
        ] {
            let encoded = apply_encode(data, kind);
            assert_eq!(encoded.chars().next().unwrap(), expected_prefix);
        }
    }

    #[test]
    fn roundtrip_base64() {
        let data = b"base64 roundtrip test \x00\xFF\xAB";
        let encoded = apply_encode(data, BASE::Base64);
        let decoded = apply_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn roundtrip_base85() {
        let data = b"base85 roundtrip test \x00\xFF\xAB";
        let encoded = apply_encode(data, BASE::Base85);
        let decoded = apply_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn roundtrip_base91() {
        let data = b"base91 roundtrip test \x00\xFF\xAB";
        let encoded = apply_encode(data, BASE::Base91);
        let decoded = apply_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn decode_empty_string_returns_none() {
        assert_eq!(apply_decode(""), None);
    }

    #[test]
    fn decode_invalid_prefix_returns_none() {
        assert_eq!(apply_decode("Xinvaliddata"), None);
    }

    #[test]
    fn roundtrip_encode_empty_payload() {
        let data: &[u8] = b"";
        let encoded = apply_encode(data, BASE::Base64);
        let decoded = apply_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    // ── Full pipeline (compress → encrypt → encode → decode → decrypt → decompress) ──

    #[test]
    fn full_pipeline_roundtrip_lz4_aes256gcm_base64() {
        let key = b"0123456789abcdef0123456789abcdef";
        let original = b"full pipeline test: compress then encrypt then encode!";

        let step1 = apply_compress(original, COMPRESS::Lz4);
        let step2 = apply_encrypt(&step1, AES::Aes256Gcm, key).unwrap();
        let step3 = apply_encode(&step2, BASE::Base64);

        let step4 = apply_decode(&step3).unwrap();
        let step5 = apply_decrypt(&step4, key).unwrap();
        let step6 = apply_decompress(&step5).unwrap();

        assert_eq!(step6, original);
    }

    #[test]
    fn full_pipeline_roundtrip_gzip_aes128ctr_base91() {
        let key = b"0123456789abcdef"; // 16 bytes
        let original = b"another pipeline: gzip + aes128ctr + base91";

        let compressed = apply_compress(original, COMPRESS::Gzip);
        let encrypted = apply_encrypt(&compressed, AES::Aes128Ctr, key).unwrap();
        let encoded = apply_encode(&encrypted, BASE::Base91);

        let decoded = apply_decode(&encoded).unwrap();
        let decrypted = apply_decrypt(&decoded, key).unwrap();
        let decompressed = apply_decompress(&decrypted).unwrap();

        assert_eq!(decompressed, original);
    }

    #[test]
    fn full_pipeline_no_compress_no_encrypt_base85() {
        let original = b"no compression, no encryption";

        let compressed = apply_compress(original, COMPRESS::None);
        let encrypted = apply_encrypt(&compressed, AES::None, b"").unwrap();
        let encoded = apply_encode(&encrypted, BASE::Base85);

        let decoded = apply_decode(&encoded).unwrap();
        let decrypted = apply_decrypt(&decoded, b"").unwrap();
        let decompressed = apply_decompress(&decrypted).unwrap();

        assert_eq!(decompressed, original);
    }
}