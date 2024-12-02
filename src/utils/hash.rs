use crate::utils::base64;
use anyhow::anyhow;
use rand::RngCore;
use ring::{
    digest::{digest, SHA256},
    hmac::{self, HMAC_SHA256},
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const SIGNED_CONTENT_SEPARATOR: &str = "@";

pub fn signing_none_secret(raw: &str) -> String {
    let digest = digest(&SHA256, raw.as_bytes());
    hex::encode(digest)
}

pub fn signed_content<T: Serialize + ?Sized>(
    content: &T,
    secret: &str,
) -> Result<String, anyhow::Error> {
    let content = base64::encode(serde_json::to_string(content)?);
    let signed_content = format!(
        "{}{}{}",
        signing(&content, secret),
        SIGNED_CONTENT_SEPARATOR,
        content
    );
    Ok(base64::encode(signed_content))
}

pub fn parse_signed_content<T: DeserializeOwned + ?Sized>(
    signed: &str,
    secret: &str,
) -> Result<T, anyhow::Error> {
    let decode = String::from_utf8(base64::decode(signed)?)?;
    let mut parts = decode.split(SIGNED_CONTENT_SEPARATOR);
    let sign = parts.next().ok_or(anyhow!("no signature decoded"))?;
    let content = parts.next().ok_or(anyhow!("no raw content decoded"))?;
    let verify = signing(content, secret);
    if verify.eq(sign) {
        let decode = base64::decode(content)?;
        return Ok(serde_json::from_slice(&decode)?);
    }
    Err(anyhow!("invalid content"))
}

pub fn signing(raw: &str, secret: &str) -> String {
    let signature = hmac::sign(
        &hmac::Key::new(HMAC_SHA256, secret.as_bytes()),
        raw.as_bytes(),
    );
    let signature = hex::encode(signature);
    signature
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedContent<T> {
    pub content: T,
    pub signed_at: i64,
    pub expire: i64,
    pub nonce: u32,
    pub id: String,
}

impl<T> SignedContent<T>
where
    T: Serialize + DeserializeOwned,
{
    pub const DEFAULT_EXPIRATION: i64 = 30;
    // const REDIS_KEY_PREFIX: &'static str = "signing:none_repeat:";
    pub fn new(content: T) -> Self {
        Self::new_with_expire(content, Self::DEFAULT_EXPIRATION)
    }

    pub fn new_with_expire(content: T, expire: i64) -> Self {
        Self {
            signed_at: chrono::Utc::now().timestamp(),
            expire,
            content,
            nonce: rand::thread_rng().next_u32(),
            id: xid::new().to_string(),
        }
    }

    pub fn to_signed_string(&self, secret: &str) -> Result<String, anyhow::Error> {
        signed_content(self, secret)
    }

    pub fn parse(signed_content: &str, secret: &str) -> Result<Self, anyhow::Error> {
        let slf: SignedContent<T> = parse_signed_content(signed_content, secret)?;
        if slf.signed_at + slf.expire <= chrono::Utc::now().timestamp() {
            return Err(anyhow!("expired content"));
        }
        Ok(slf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_signature() {
        let raw = "string for signing";
        let secret = "secret";
        let _ = signing_none_secret(raw);
        let signed = signed_content(raw, secret).unwrap();
        let parsed: String = parse_signed_content(&signed, secret).unwrap();
        assert_eq!(parsed, raw);
        let signed_content = SignedContent::new(raw.to_owned());
        let signed = signed_content.to_signed_string(secret).unwrap();
        let parse: SignedContent<String> = SignedContent::parse(&signed, secret).unwrap();
        assert_eq!(parse.content, raw);
    }
}
