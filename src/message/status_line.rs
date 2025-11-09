use std::io::{Result, Write};

#[derive(Debug)]
pub enum StatusCode {
    Ok,                  // 200
    BadRequest,          // 400
    NotFound,            // 404
    MethodNotAllowed,    // 405
    InternalServerError, // 500
}

impl StatusCode {
    pub fn to_bytes(&self) -> String {
        match self {
            Self::Ok => "200",
            Self::BadRequest => "400",
            Self::NotFound => "404",
            Self::MethodNotAllowed => "405",
            Self::InternalServerError => "500",
        }
        .to_string()
    }
    pub fn to_reason(&self) -> String {
        match self {
            Self::Ok => "Ok",
            Self::BadRequest => "Bad Request",
            Self::NotFound => "Not Found",
            Self::MethodNotAllowed => "Method Not Allowed",
            Self::InternalServerError => "Internal Server Error",
        }
        .to_string()
    }
}

#[derive(Debug)]
pub struct StatusLine {
    version: String,
    status_code: StatusCode,
}

impl StatusLine {
    pub fn new(status_code: StatusCode) -> StatusLine {
        StatusLine {
            version: "1.1".to_string(),
            status_code,
        }
    }

    /// Follows RFC 9112
    /// Sp = Single Space
    ///
    /// status-line = HTTP-version SP status-code SP [ reason-phrase ]
    ///
    /// # Errors
    ///
    /// Returns Error if write fails
    pub fn write_to<W: Write>(&self, mut w: W) -> Result<()> {
        write!(
            w,
            "HTTP/{} {} {}\r\n",
            self.version,
            self.status_code.to_bytes(),
            self.status_code.to_reason()
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_line_write_to() -> Result<()> {
        let status_line = StatusLine::new(StatusCode::Ok);
        let mut buf = Vec::new();
        status_line.write_to(&mut buf)?;
        assert_eq!(buf, b"HTTP/1.1 200 Ok\r\n".to_vec());

        Ok(())
    }
}
