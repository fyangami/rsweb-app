pub use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64;
use base64::Engine;

pub fn encode<T: AsRef<[u8]>>(raw: T) -> String {
    BASE64.encode(raw)
}

pub fn decode<T: AsRef<[u8]>>(encode: T) -> Result<Vec<u8>, base64::DecodeError> {
    BASE64.decode::<T>(encode)
}