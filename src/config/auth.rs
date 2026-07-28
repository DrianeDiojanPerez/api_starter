#[derive(Debug, Clone)]
pub struct Auth {
    pub access_token_ttl_in_seconds: i64,
    pub refresh_token_ttl_in_seconds: i64,
}
