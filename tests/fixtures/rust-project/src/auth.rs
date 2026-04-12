use crate::models::User;

pub fn login(username: &str, password: &str) -> Option<User> {
    let user = find_user(username)?;
    if verify_password(&user, password) {
        Some(user)
    } else {
        None
    }
}

fn find_user(username: &str) -> Option<User> {
    if username == "admin" {
        Some(User {
            id: 1,
            name: "Admin".to_string(),
            role: "admin".to_string(),
        })
    } else {
        None
    }
}

fn verify_password(user: &User, password: &str) -> bool {
    let _ = user;
    password == "secret"
}

pub fn logout(user: &User) {
    println!("Logging out: {}", user.name);
}
