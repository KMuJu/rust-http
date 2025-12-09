use std::fmt::Display;

use crate::message::error::VersionError;

#[derive(Debug, PartialEq, Eq)]
pub struct HttpVersion(u8, u8);

impl HttpVersion {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, VersionError> {
        match bytes {
            b"1.0" => Ok(Self(1, 0)),
            b"1.1" => Ok(Self(1, 1)),
            _ => Err(VersionError::InvalidHTTPVersion),
        }
    }
}

impl Default for HttpVersion {
    fn default() -> Self {
        Self(1, 1)
    }
}

impl PartialEq<(u8, u8)> for HttpVersion {
    fn eq(&self, other: &(u8, u8)) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl PartialEq<HttpVersion> for (u8, u8) {
    fn eq(&self, other: &HttpVersion) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl From<(u8, u8)> for HttpVersion {
    fn from(value: (u8, u8)) -> Self {
        Self(value.0, value.1)
    }
}

impl Display for HttpVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match (self.0, self.1) {
                (1, 0) => "1.0".to_string(),
                (1, 1) => "1.1".to_string(),
                (_, _) => "".to_string(),
            }
        )
    }
}
