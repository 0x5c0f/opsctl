use sha2::Digest;

/// HMAC over hash `D` with the given block size (SHA-1/SHA-256 both use 64).
fn hmac<D: Digest>(key: &[u8], msg: &[u8], block_size: usize) -> Vec<u8> {
    let mut k = vec![0u8; block_size];
    if key.len() > block_size {
        let hashed = D::digest(key);
        k[..hashed.len()].copy_from_slice(&hashed);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut ipad = vec![0x36u8; block_size];
    let mut opad = vec![0x5cu8; block_size];
    for i in 0..block_size {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }

    let mut inner = D::new();
    inner.update(&ipad);
    inner.update(msg);
    let inner_hash = inner.finalize();

    let mut outer = D::new();
    outer.update(&opad);
    outer.update(inner_hash);
    outer.finalize().to_vec()
}

pub fn hmac_sha1(key: &[u8], msg: &[u8]) -> Vec<u8> {
    hmac::<sha1::Sha1>(key, msg, 64)
}

pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    hmac::<sha2::Sha256>(key, msg, 64)
}

/// RFC3986 percent-encode: only A-Z a-z 0-9 - _ . ~ remain unescaped.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_sha1_matches_rfc2202_vector() {
        let key = vec![0x0b; 20];
        let got = hmac_sha1(&key, b"Hi There");
        let hex = hex::encode(got);
        assert_eq!(hex, "b617318655057264e28bc0b6fb378c8ef146be00");
    }

    #[test]
    fn hmac_sha256_matches_rfc4231_vector() {
        let key = vec![0x0b; 20];
        let got = hmac_sha256(&key, b"Hi There");
        let hex = hex::encode(got);
        assert_eq!(
            hex,
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn percent_encode_keeps_unreserved_and_uppercases_escapes() {
        assert_eq!(percent_encode("A-Za-z0-9-_.~"), "A-Za-z0-9-_.~");
        assert_eq!(percent_encode("a b/c?d"), "a%20b%2Fc%3Fd");
    }
}
