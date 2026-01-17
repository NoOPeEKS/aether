use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Eq, Hash, PartialEq)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub permissions: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct JWTClaims {
    // Standard claims
    pub sub: String, // will be user_id
    pub exp: usize,
    pub iat: usize,
    pub iss: String,

    pub user_id: Uuid,
    pub username: String,
    pub is_admin: bool,
    pub permissions: Vec<String>,
}
