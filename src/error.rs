use std::fmt;

/// Error returned by SEDMAT parsing, planning, or execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SedmatError(pub String);

impl SedmatError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

impl fmt::Display for SedmatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SedmatError {}

pub type Result<T> = std::result::Result<T, SedmatError>;
