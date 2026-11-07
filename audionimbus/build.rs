fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(static_api)");
    println!("cargo::rustc-check-cfg=cfg(dynamic_api)");

    let static_api = std::env::var_os("CARGO_FEATURE_STATIC").is_some()
        || std::env::var_os("CARGO_FEATURE_DYNAMIC").is_none();

    if static_api {
        println!("cargo::rustc-cfg=static_api");
    } else {
        println!("cargo::rustc-cfg=dynamic_api");
    }
}
