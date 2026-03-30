//src/binary_data_process/z_base.rs

pub trait Encoder {
    fn encode<T: AsRef<[u8]>>(input: T) -> String;
    fn decode(input: &str) -> Option<Vec<u8>>;
}

const fn build_decode_table<const N: usize>(alphabet: &[u8; N]) -> [u8; 256] {
    let mut table = [0xFFu8; 256];
    let mut i = 0;
    while i < N {
        table[alphabet[i] as usize] = i as u8;
        i += 1;
    }
    table
}

const fn validate_alphabet<const N: usize>(alphabet: &[u8; N]) {
    let mut i = 0;
    while i < N {
        assert!(
            alphabet[i] >= 0x21 && alphabet[i] <= 0x7E,
            "Alphabet contains non-printable character"
        );
        let mut j = i + 1;
        while j < N {
            assert!(
                alphabet[i] != alphabet[j],
                "Alphabet contains duplicate character"
            );
            j += 1;
        }
        i += 1;
    }
}

const BASE64_ALPHABET: &[u8; 64] =
    b"0123456789abcdefghIJKLMNOPQRSTUVWXYZABCDEFGHijklmnopqrstuvwxyz+/";

const _: () = validate_alphabet(BASE64_ALPHABET);

const BASE64_DECODE_TABLE: [u8; 256] = build_decode_table(BASE64_ALPHABET);

pub struct Base64;

impl Base64 {
    #[inline(always)]
    fn decode_char(c: u8) -> Option<u8> {
        let v = BASE64_DECODE_TABLE[c as usize];
        if v == 0xFF { None } else { Some(v) }
    }
}

impl Encoder for Base64 {
    fn encode<T: AsRef<[u8]>>(input: T) -> String {
        let input = input.as_ref();
        if input.is_empty() {
            return String::new();
        }

        let out_len = (input.len() + 2) / 3 * 4;
        let mut buf: Vec<u8> = Vec::with_capacity(out_len);
        let full_chunks = input.len() / 3;
        let remainder = input.len() % 3;

        for i in 0..full_chunks {
            let off = i * 3;
            let n =
                (input[off] as u32) << 16 | (input[off + 1] as u32) << 8 | input[off + 2] as u32;
            buf.push(BASE64_ALPHABET[(n >> 18 & 0x3F) as usize]);
            buf.push(BASE64_ALPHABET[(n >> 12 & 0x3F) as usize]);
            buf.push(BASE64_ALPHABET[(n >> 6 & 0x3F) as usize]);
            buf.push(BASE64_ALPHABET[(n & 0x3F) as usize]);
        }

        match remainder {
            2 => {
                let off = full_chunks * 3;
                let n = (input[off] as u32) << 16 | (input[off + 1] as u32) << 8;
                buf.push(BASE64_ALPHABET[(n >> 18 & 0x3F) as usize]);
                buf.push(BASE64_ALPHABET[(n >> 12 & 0x3F) as usize]);
                buf.push(BASE64_ALPHABET[(n >> 6 & 0x3F) as usize]);
                buf.push(b'=');
            }
            1 => {
                let off = full_chunks * 3;
                let n = (input[off] as u32) << 16;
                buf.push(BASE64_ALPHABET[(n >> 18 & 0x3F) as usize]);
                buf.push(BASE64_ALPHABET[(n >> 12 & 0x3F) as usize]);
                buf.push(b'=');
                buf.push(b'=');
            }
            _ => {}
        }

        unsafe { String::from_utf8_unchecked(buf) }
    }

    fn decode(input: &str) -> Option<Vec<u8>> {
        let bytes = input.as_bytes();

        let effective_len = bytes.iter().filter(|&&b| !b.is_ascii_whitespace()).count();
        if effective_len == 0 {
            return Some(Vec::new());
        }
        if effective_len % 4 != 0 {
            return None;
        }

        let total_chunks = effective_len / 4;
        let mut result = Vec::with_capacity(total_chunks * 3);

        let mut chunk = [0u8; 4];
        let mut pos = 0usize;
        let mut chunk_idx = 0usize;

        for &b in bytes {
            if b.is_ascii_whitespace() {
                continue;
            }
            chunk[pos] = b;
            pos += 1;

            if pos == 4 {
                chunk_idx += 1;
                let is_last = chunk_idx == total_chunks;
                let pad_count = chunk.iter().filter(|&&c| c == b'=').count();

                if pad_count > 0 {
                    if !is_last || pad_count > 2 {
                        return None;
                    }
                    let data_chars = 4 - pad_count;

                    for idx in 0..data_chars {
                        if chunk[idx] == b'=' {
                            return None;
                        }
                    }
                    for idx in data_chars..4 {
                        if chunk[idx] != b'=' {
                            return None;
                        }
                    }
                }

                let v0 = if chunk[0] == b'=' {
                    0u32
                } else {
                    Self::decode_char(chunk[0])? as u32
                };
                let v1 = if chunk[1] == b'=' {
                    0u32
                } else {
                    Self::decode_char(chunk[1])? as u32
                };
                let v2 = if chunk[2] == b'=' {
                    0u32
                } else {
                    Self::decode_char(chunk[2])? as u32
                };
                let v3 = if chunk[3] == b'=' {
                    0u32
                } else {
                    Self::decode_char(chunk[3])? as u32
                };

                let n = v0 << 18 | v1 << 12 | v2 << 6 | v3;

                if pad_count == 2 && (n & 0xFFFF) != 0 {
                    return None;
                }
                if pad_count == 1 && (n & 0xFF) != 0 {
                    return None;
                }

                result.push((n >> 16) as u8);
                if pad_count < 2 {
                    result.push((n >> 8 & 0xFF) as u8);
                }
                if pad_count < 1 {
                    result.push((n & 0xFF) as u8);
                }

                pos = 0;
            }
        }

        Some(result)
    }
}

pub struct Base85;

impl Base85 {
    #[inline(always)]
    fn char_to_digit(c: u8) -> Option<u8> {
        if c >= 33 && c <= 117 {
            Some(c - 33)
        } else {
            None
        }
    }
}

impl Encoder for Base85 {
    fn encode<T: AsRef<[u8]>>(input: T) -> String {
        let input = input.as_ref();

        let estimated = input.len() / 4 * 5 + 6 + 4;
        let mut buf: Vec<u8> = Vec::with_capacity(estimated);
        buf.extend_from_slice(b"<~");

        for chunk in input.chunks(4) {
            let mut acc: u32 = 0;
            for (i, &byte) in chunk.iter().enumerate() {
                acc |= (byte as u32) << (24 - i * 8);
            }

            if chunk.len() == 4 && acc == 0 {
                buf.push(b'z');
                continue;
            }

            let mut digits = [0u8; 5];
            for i in (0..5).rev() {
                digits[i] = (acc % 85) as u8;
                acc /= 85;
            }

            let count = chunk.len() + 1;
            for &d in &digits[..count] {
                buf.push(d + 33);
            }
        }

        buf.extend_from_slice(b"~>");

        unsafe { String::from_utf8_unchecked(buf) }
    }

    fn decode(input: &str) -> Option<Vec<u8>> {
        let bytes = input.as_bytes();

        let stripped: Vec<u8> = bytes
            .iter()
            .copied()
            .filter(|b| !b.is_ascii_whitespace())
            .collect();
        let data = if stripped.starts_with(b"<~") && stripped.ends_with(b"~>") {
            &stripped[2..stripped.len() - 2]
        } else {
            &stripped
        };

        let mut result = Vec::with_capacity(data.len() / 5 * 4 + 4);
        let mut i = 0;

        while i < data.len() {
            if data[i] == b'z' {
                result.extend_from_slice(&[0, 0, 0, 0]);
                i += 1;
                continue;
            }

            let remaining = data.len() - i;
            let count = remaining.min(5);

            if count < 2 {
                return None;
            }

            if count < 5 && i + count != data.len() {
                return None;
            }

            let mut block = [84u8; 5];
            for j in 0..count {
                block[j] = Self::char_to_digit(data[i + j])?;
            }

            let mut acc: u32 = 0;
            for &d in &block {
                acc = acc.checked_mul(85)?.checked_add(d as u32)?;
            }

            let out_bytes = count - 1;
            let all_bytes = acc.to_be_bytes();
            result.extend_from_slice(&all_bytes[..out_bytes]);

            i += count;
        }

        Some(result)
    }
}

const BASE91_ALPHABET: &[u8; 91] =
    b"abcdefghABCDEFGH0123456789IJKLMNOPQRSTUVWXYZijklmnopqrstuvwxyz!#$%&()*+,./:;<=>?@[]^_`{|}~\"";

const _: () = validate_alphabet(BASE91_ALPHABET);

const BASE91_DECODE_TABLE: [u8; 256] = build_decode_table(BASE91_ALPHABET);

pub struct Base91;

impl Base91 {
    #[inline(always)]
    fn decode_char(c: u8) -> Option<u8> {
        let v = BASE91_DECODE_TABLE[c as usize];
        if v == 0xFF { None } else { Some(v) }
    }
}

impl Encoder for Base91 {
    fn encode<T: AsRef<[u8]>>(input: T) -> String {
        let input = input.as_ref();

        let mut buf: Vec<u8> = Vec::with_capacity(input.len().saturating_mul(2).saturating_add(2));

        let mut n: u32 = 0;
        let mut bits: u32 = 0;

        for &byte in input {
            n |= (byte as u32) << bits;
            bits += 8;

            if bits > 13 {
                let mut val = n & 8191;
                if val > 88 {
                    n >>= 13;
                    bits -= 13;
                } else {
                    val = n & 16383;
                    n >>= 14;
                    bits -= 14;
                }
                buf.push(BASE91_ALPHABET[(val % 91) as usize]);
                buf.push(BASE91_ALPHABET[(val / 91) as usize]);
            }
        }

        if bits > 0 {
            buf.push(BASE91_ALPHABET[(n % 91) as usize]);
            if bits > 7 || n > 90 {
                buf.push(BASE91_ALPHABET[(n / 91) as usize]);
            }
        }

        unsafe { String::from_utf8_unchecked(buf) }
    }

    fn decode(input: &str) -> Option<Vec<u8>> {
        let bytes = input.as_bytes();
        let mut result = Vec::with_capacity(bytes.len());

        let mut n: u32 = 0;
        let mut bits: u32 = 0;
        let mut val: i32 = -1;

        for &c in bytes {
            let d = Self::decode_char(c)? as u32;

            if val == -1 {
                val = d as i32;
            } else {
                val += (d as i32) * 91;
                n |= (val as u32) << bits;
                bits += if (val & 8191) > 88 { 13 } else { 14 };

                while bits >= 8 {
                    result.push((n & 0xFF) as u8);
                    n >>= 8;
                    bits -= 8;
                }
                val = -1;
            }
        }

        if val != -1 {
            n |= (val as u32) << bits;
            result.push((n & 0xFF) as u8);
        }

        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use base64::Engine;

    #[test]
    fn test_base64_performance() {
        let input = vec![0u8; 1024 * 1024]; // 1 MB 测试数据

        // 自定义 Base64 encode
        let start = Instant::now();
        let encoded_custom = Base64::encode(&input);
        let duration_custom_encode = start.elapsed();
        println!("Custom Base64 encode: {:?}", duration_custom_encode);

        // 自定义 Base64 decode
        let start = Instant::now();
        let decoded_custom = Base64::decode(&encoded_custom).expect("Decode failed");
        let duration_custom_decode = start.elapsed();
        println!("Custom Base64 decode: {:?}", duration_custom_decode);

        assert_eq!(decoded_custom, input);

        // 标准库 Base64 encode
        let start = Instant::now();
        let encoded_std = base64::engine::general_purpose::STANDARD.encode(&input);
        let duration_std_encode = start.elapsed();
        println!("Std Base64 encode: {:?}", duration_std_encode);

        // 标准库 Base64 decode
        let start = Instant::now();
        let decoded_std = base64::engine::general_purpose::STANDARD
            .decode(&encoded_std)
            .expect("Std decode failed");
        let duration_std_decode = start.elapsed();
        println!("Std Base64 decode: {:?}", duration_std_decode);

        assert_eq!(decoded_std, input);

        // 对比性能
        println!(
            "Encode speed ratio (custom / std): {:.2}",
            duration_std_encode.as_secs_f64() / duration_custom_encode.as_secs_f64()
        );
        println!(
            "Decode speed ratio (custom / std): {:.2}",
            duration_std_decode.as_secs_f64() / duration_custom_decode.as_secs_f64()
        );
    }
}