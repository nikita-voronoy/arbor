#[derive(Debug, Clone)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub role: String,
}

pub struct Session {
    pub user: User,
    pub token: String,
}

pub enum AuthError {
    InvalidCredentials,
    UserNotFound,
    SessionExpired,
}
