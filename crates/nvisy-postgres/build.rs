#![forbid(unsafe_code)]

/// Due to limitations in Rust's proc-macro API, there is currently no way to
/// signal that a specific procedural macro should be rerun if
/// an external file changes or is added.
///
/// This means that `embed_migrations!` cannot automatically regenerate
/// the list of embedded migrations if only the migration files are updated.
///
/// To work around this limitation, you can add a custom `build.rs` file to your crate.
/// This ensures the crate is rebuilt whenever the migration directory changes.
fn main() {
    println!("cargo:rerun-if-changed=./src/migrations");
}
