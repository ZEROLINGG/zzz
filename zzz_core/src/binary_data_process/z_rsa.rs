// src/binary_data_process/z_rsa.rs

use rsa::{RsaPrivateKey, RsaPublicKey};

// ─── Trait 定义 ───────────────────────────────────────────────────────────────

pub trait AsymmetricCipher {
    /// 生成密钥对，返回 (公钥字节, 私钥字节)，使用 DER 格式
    fn generate_keypair() -> Option<(Vec<u8>, Vec<u8>)>;
    fn encrypt<T: AsRef<[u8]>>(public_key: &[u8], plaintext: T) -> Option<Vec<u8>>;
    fn decrypt<T: AsRef<[u8]>>(private_key: &[u8], ciphertext: T) -> Option<Vec<u8>>;
    fn sign<T: AsRef<[u8]>>(private_key: &[u8], message: T) -> Option<Vec<u8>>;
    fn verify<T: AsRef<[u8]>>(public_key: &[u8], message: T, signature: &[u8]) -> bool;
}

// ─── DER 解析辅助宏 ───────────────────────────────────────────────────────────

/// 从 DER 字节解析公钥，失败返回 None
macro_rules! parse_public_key {
    ($bytes:expr) => {{
        use rsa::pkcs8::DecodePublicKey;
        RsaPublicKey::from_public_key_der($bytes).ok()
    }};
}

/// 从 DER 字节解析私钥，失败返回 None
macro_rules! parse_private_key {
    ($bytes:expr) => {{
        use rsa::pkcs8::DecodePrivateKey;
        RsaPrivateKey::from_pkcs8_der($bytes).ok()
    }};
}

// ─── RSA-2048 ─────────────────────────────────────────────────────────────────

pub struct Rsa2048;

impl AsymmetricCipher for Rsa2048 {
    fn generate_keypair() -> Option<(Vec<u8>, Vec<u8>)> {
        use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};
        use rand::rngs::OsRng;

        let private_key = RsaPrivateKey::new(&mut OsRng, 2048).ok()?;
        let public_key = RsaPublicKey::from(&private_key);

        let pub_der = public_key.to_public_key_der().ok()?.to_vec();
        let pri_der = private_key.to_pkcs8_der().ok()?.as_bytes().to_vec();
        Some((pub_der, pri_der))
    }

    fn encrypt<T: AsRef<[u8]>>(public_key: &[u8], plaintext: T) -> Option<Vec<u8>> {
        use rsa::Oaep;
        use rand::rngs::OsRng;

        let key = parse_public_key!(public_key)?;
        key.encrypt(&mut OsRng, Oaep::new::<sha2::Sha256>(), plaintext.as_ref()).ok()
    }

    fn decrypt<T: AsRef<[u8]>>(private_key: &[u8], ciphertext: T) -> Option<Vec<u8>> {
        use rsa::Oaep;

        let key = parse_private_key!(private_key)?;
        key.decrypt(Oaep::new::<sha2::Sha256>(), ciphertext.as_ref()).ok()
    }

    fn sign<T: AsRef<[u8]>>(private_key: &[u8], message: T) -> Option<Vec<u8>> {
        use rsa::pss::{BlindedSigningKey, Signature};
        use rsa::signature::{RandomizedSigner, SignatureEncoding};
        use rand::rngs::OsRng;

        let key = parse_private_key!(private_key)?;
        let signing_key = BlindedSigningKey::<sha2::Sha256>::new(key);
        let sig: Signature = signing_key.sign_with_rng(&mut OsRng, message.as_ref());
        Some(sig.to_vec())
    }

    fn verify<T: AsRef<[u8]>>(public_key: &[u8], message: T, signature: &[u8]) -> bool {
        use rsa::pss::{Signature, VerifyingKey};
        use rsa::signature::Verifier;

        let key = match parse_public_key!(public_key) {
            Some(k) => k,
            None => return false,
        };
        let verifying_key = VerifyingKey::<sha2::Sha256>::new(key);
        let sig = match Signature::try_from(signature) {
            Ok(s) => s,
            Err(_) => return false,
        };
        verifying_key.verify(message.as_ref(), &sig).is_ok()
    }
}

// ─── RSA-4096 ─────────────────────────────────────────────────────────────────

pub struct Rsa4096;

impl AsymmetricCipher for Rsa4096 {
    fn generate_keypair() -> Option<(Vec<u8>, Vec<u8>)> {
        use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};
        use rand::rngs::OsRng;

        let private_key = RsaPrivateKey::new(&mut OsRng, 4096).ok()?;
        let public_key = RsaPublicKey::from(&private_key);

        let pub_der = public_key.to_public_key_der().ok()?.to_vec();
        let pri_der = private_key.to_pkcs8_der().ok()?.as_bytes().to_vec();
        Some((pub_der, pri_der))
    }

    fn encrypt<T: AsRef<[u8]>>(public_key: &[u8], plaintext: T) -> Option<Vec<u8>> {
        use rsa::Oaep;
        use rand::rngs::OsRng;

        let key = parse_public_key!(public_key)?;
        key.encrypt(&mut OsRng, Oaep::new::<sha2::Sha512>(), plaintext.as_ref()).ok()
    }

    fn decrypt<T: AsRef<[u8]>>(private_key: &[u8], ciphertext: T) -> Option<Vec<u8>> {
        use rsa::Oaep;

        let key = parse_private_key!(private_key)?;
        key.decrypt(Oaep::new::<sha2::Sha512>(), ciphertext.as_ref()).ok()
    }

    fn sign<T: AsRef<[u8]>>(private_key: &[u8], message: T) -> Option<Vec<u8>> {
        use rsa::pss::{BlindedSigningKey, Signature};
        use rsa::signature::{RandomizedSigner, SignatureEncoding};
        use rand::rngs::OsRng;

        let key = parse_private_key!(private_key)?;
        let signing_key = BlindedSigningKey::<sha2::Sha512>::new(key);
        let sig: Signature = signing_key.sign_with_rng(&mut OsRng, message.as_ref());
        Some(sig.to_vec())
    }

    fn verify<T: AsRef<[u8]>>(public_key: &[u8], message: T, signature: &[u8]) -> bool {
        use rsa::pss::{Signature, VerifyingKey};
        use rsa::signature::Verifier;

        let key = match parse_public_key!(public_key) {
            Some(k) => k,
            None => return false,
        };
        let verifying_key = VerifyingKey::<sha2::Sha512>::new(key);
        let sig = match Signature::try_from(signature) {
            Ok(s) => s,
            Err(_) => return false,
        };
        verifying_key.verify(message.as_ref(), &sig).is_ok()
    }
}

// ─── 单元测试 ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"Permissions of this weak copyleft license are conditioned on making \
        available source code of licensed files and modifications of those files \
        under the same license (or in certain cases, one of the GNU licenses).";

    // 每次生成密钥对耗时较长，用 once_cell 缓存，避免重复生成拖慢测试
    use std::sync::OnceLock;

    static KEYPAIR_2048: OnceLock<(Vec<u8>, Vec<u8>)> = OnceLock::new();
    static KEYPAIR_4096: OnceLock<(Vec<u8>, Vec<u8>)> = OnceLock::new();

    fn keypair_2048() -> &'static (Vec<u8>, Vec<u8>) {
        KEYPAIR_2048.get_or_init(|| Rsa2048::generate_keypair().expect("keygen failed"))
    }

    fn keypair_4096() -> &'static (Vec<u8>, Vec<u8>) {
        KEYPAIR_4096.get_or_init(|| Rsa4096::generate_keypair().expect("keygen failed"))
    }

    fn round_trip_encrypt<C: AsymmetricCipher>(
        label: &str,
        pub_key: &[u8],
        pri_key: &[u8],
        plaintext: &[u8],
    ) {
        let encrypted = C::encrypt(pub_key, plaintext).expect("encrypt failed");
        let decrypted = C::decrypt(pri_key, &encrypted).expect("decrypt failed");
        assert_eq!(decrypted, plaintext, "{label}: encrypt round-trip mismatch");
        println!(
            "{label}: {} -> {} bytes ({:.1}%)",
            plaintext.len(),
            encrypted.len(),
            encrypted.len() as f64 / plaintext.len() as f64 * 100.0
        );
    }

    fn round_trip_sign<C: AsymmetricCipher>(
        label: &str,
        pub_key: &[u8],
        pri_key: &[u8],
        message: &[u8],
    ) {
        let signature = C::sign(pri_key, message).expect("sign failed");
        let ok = C::verify(pub_key, message, &signature);
        assert!(ok, "{label}: signature verification failed");
        println!("{label}: signature {} bytes", signature.len());
    }

    // ── RSA-2048 ──────────────────────────────────────────────────────────────

    #[test]
    fn test_rsa2048_encrypt_round_trip() {
        // RSA-2048/OAEP-SHA256 单次最大明文 = (2048/8) - 2*32 - 2 = 190 字节
        let (pub_key, pri_key) = keypair_2048();
        round_trip_encrypt::<Rsa2048>("rsa-2048-oaep", pub_key, pri_key, &SAMPLE[..64]);
    }

    #[test]
    fn test_rsa2048_sign_round_trip() {
        let (pub_key, pri_key) = keypair_2048();
        round_trip_sign::<Rsa2048>("rsa-2048-pss", pub_key, pri_key, SAMPLE);
    }

    #[test]
    fn test_rsa2048_wrong_key_decrypt_returns_none() {
        let (pub_key, _) = keypair_2048();
        let (_, other_pri) = Rsa2048::generate_keypair().expect("keygen failed");
        let encrypted = Rsa2048::encrypt(pub_key, &SAMPLE[..64]).expect("encrypt failed");
        assert!(
            Rsa2048::decrypt(&other_pri, &encrypted).is_none(),
            "rsa-2048: wrong private key must return None"
        );
    }

    #[test]
    fn test_rsa2048_tampered_signature_fails() {
        let (pub_key, pri_key) = keypair_2048();
        let mut sig = Rsa2048::sign(pri_key, SAMPLE).expect("sign failed");
        // 篡改签名最后一个字节
        *sig.last_mut().unwrap() ^= 0xFF;
        assert!(
            !Rsa2048::verify(pub_key, SAMPLE, &sig),
            "rsa-2048: tampered signature must not verify"
        );
    }

    #[test]
    fn test_rsa2048_invalid_key_bytes() {
        assert!(Rsa2048::encrypt(&[0u8; 32], SAMPLE).is_none());
        assert!(Rsa2048::decrypt(&[0u8; 32], &[0u8; 256]).is_none());
        assert!(Rsa2048::sign(&[0u8; 32], SAMPLE).is_none());
        assert!(!Rsa2048::verify(&[0u8; 32], SAMPLE, &[0u8; 256]));
    }

    // ── RSA-4096 ──────────────────────────────────────────────────────────────

    #[test]
    fn test_rsa4096_encrypt_round_trip() {
        // RSA-4096/OAEP-SHA512 单次最大明文 = (4096/8) - 2*64 - 2 = 382 字节
        let (pub_key, pri_key) = keypair_4096();
        round_trip_encrypt::<Rsa4096>("rsa-4096-oaep", pub_key, pri_key, &SAMPLE[..64]);
    }

    #[test]
    fn test_rsa4096_sign_round_trip() {
        let (pub_key, pri_key) = keypair_4096();
        round_trip_sign::<Rsa4096>("rsa-4096-pss", pub_key, pri_key, SAMPLE);
    }

    #[test]
    fn test_rsa4096_tampered_signature_fails() {
        let (pub_key, pri_key) = keypair_4096();
        let mut sig = Rsa4096::sign(pri_key, SAMPLE).expect("sign failed");
        *sig.last_mut().unwrap() ^= 0xFF;
        assert!(
            !Rsa4096::verify(pub_key, SAMPLE, &sig),
            "rsa-4096: tampered signature must not verify"
        );
    }

    #[test]
    fn test_rsa4096_invalid_key_bytes() {
        assert!(Rsa4096::encrypt(&[0u8; 32], SAMPLE).is_none());
        assert!(Rsa4096::decrypt(&[0u8; 32], &[0u8; 512]).is_none());
        assert!(Rsa4096::sign(&[0u8; 32], SAMPLE).is_none());
        assert!(!Rsa4096::verify(&[0u8; 32], SAMPLE, &[0u8; 512]));
    }
}