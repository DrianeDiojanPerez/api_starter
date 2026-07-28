use uuid::Uuid;

/// Identity shared across modules. The password hash never leaves the auth
/// layer, so it is skipped during serialization.
#[derive(Debug, Clone, serde::Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub user_name: String,
    #[serde(skip)]
    pub password: String,
    pub roles: Vec<String>,
}
