//src/binary_data_process/z_compress.rs

pub trait Compressor {
    fn compress<T: AsRef<[u8]>>(input: T) -> Vec<u8>;
    fn decompress(input: &[u8]) -> Option<Vec<u8>>;
}


pub struct Lz4;

impl Compressor for Lz4 {
    fn compress<T: AsRef<[u8]>>(input: T) -> Vec<u8> {
        lz4_flex::compress_prepend_size(input.as_ref())
    }

    fn decompress(input: &[u8]) -> Option<Vec<u8>> {
        lz4_flex::decompress_size_prepended(input).ok()
    }
}


pub struct Gzip;

impl Compressor for Gzip {
    fn compress<T: AsRef<[u8]>>(input: T) -> Vec<u8> {
        use flate2::{write::GzEncoder, Compression};
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(input.as_ref())
            .expect("gzip compress write failed");
        encoder.finish().expect("gzip compress finish failed")
    }

    fn decompress(input: &[u8]) -> Option<Vec<u8>> {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(input);
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf).ok()?;
        Some(buf)
    }
}


pub struct Zstd;

impl Compressor for Zstd {
    fn compress<T: AsRef<[u8]>>(input: T) -> Vec<u8> {
        // level 3 为 zstd 默认等级，兼顾速度与压缩率
        zstd::encode_all(input.as_ref(), 3).expect("zstd compress failed")
    }

    fn decompress(input: &[u8]) -> Option<Vec<u8>> {
        zstd::decode_all(input).ok()
    }
}

// ─── 单元测试 ────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"Permissions of this weak copyleft license are conditioned on making available source code of licensed files and modifications of those files under the same license (or in certain cases, one of the GNU licenses). Copyright and license notices must be preserved. Contributors provide an express grant of patent rights. However, a larger work using the licensed work may be distributed under different terms and without source code for files added in the larger work.";

    fn round_trip<C: Compressor>(label: &str) {
        let compressed = C::compress(SAMPLE);
        let decompressed = C::decompress(&compressed).expect("decompress failed");
        assert_eq!(decompressed, SAMPLE, "{label}: round-trip mismatch");
        println!(
            "{label}: {} -> {} bytes ({:.1}%)",
            SAMPLE.len(),
            compressed.len(),
            compressed.len() as f64 / SAMPLE.len() as f64 * 100.0
        );
    }

    #[test]
    fn test_lz4()  { round_trip::<Lz4>("lz4");   }
    #[test]
    fn test_gzip() { round_trip::<Gzip>("gzip");  }
    #[test]
    fn test_zstd() { round_trip::<Zstd>("zstd");  }
}