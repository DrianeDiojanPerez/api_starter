use crate::package::masked::MaskedBytes;

#[derive(Debug, Clone)]
pub struct Jwt {
    secret: MaskedBytes,
}

impl Jwt {
    pub fn new(secret: impl Into<Vec<u8>>) -> Self {
        Self {
            secret: MaskedBytes::new(secret),
        }
    }

    pub fn secret(&self) -> &MaskedBytes {
        &self.secret
    }
}
