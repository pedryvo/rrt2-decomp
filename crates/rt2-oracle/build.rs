fn main() {
    // The bundled (static) Unicorn needs 16-byte atomics from libatomic on Linux.
    let bundled = std::env::var_os("CARGO_FEATURE_SYSTEM_UNICORN").is_none();
    if bundled && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-lib=atomic");
    }
}
