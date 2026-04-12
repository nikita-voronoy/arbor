mod auth;
mod models;

fn main() {
    let user = auth::login("admin", "secret");
    println!("Logged in: {:?}", user);
}
