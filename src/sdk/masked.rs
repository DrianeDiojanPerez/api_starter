use std::fmt;

/// Secret bytes that never leak through `Debug`, `Display` or JSON output.
#[derive(Clone)]
pub struct MaskedBytes(Vec<u8>);

impl MaskedBytes {
    pub fn new(value: impl Into<Vec<u8>>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for MaskedBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("************")
    }
}

impl fmt::Display for MaskedBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("************")
    }
}

impl serde::Serialize for MaskedBytes {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("********")
    }
}

/// Placeholder used when redacting a field inside a `Debug` implementation.
pub struct MaskedString;

impl fmt::Debug for MaskedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("************")
    }
}
