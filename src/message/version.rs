use std::fmt::Display;

use crate::message::error::VersionError;

#[derive(Debug, PartialEq, Eq)]
pub struct HttpVersion(u8, u8);

impl HttpVersion {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, VersionError> {
        match bytes {
            b"1.0" => Ok(Self(1, 0)),
            b"1.1" => Ok(Self(1, 1)),
            b"2.0" => Ok(Self(2, 0)),
            b"3.0" => Ok(Self(3, 0)),
            _ => Err(VersionError::InvalidHTTPVersion),
        }
    }

    pub fn new(major: u8, minor: u8) -> Self {
        Self(major, minor)
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

impl PartialOrd<HttpVersion> for HttpVersion {
    fn partial_cmp(&self, other: &HttpVersion) -> Option<std::cmp::Ordering> {
        match self.0.partial_cmp(&other.0) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.1.partial_cmp(&other.1)
    }
}

impl From<(u8, u8)> for HttpVersion {
    fn from(value: (u8, u8)) -> Self {
        Self(value.0, value.1)
    }
}

impl Display for HttpVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.0, self.1)
    }
}
