fn main() {
    // sqlx::migrate! embeds ../migrations: rebuild when a migration is added or changed.
    println!("cargo:rerun-if-changed=../migrations");
}
