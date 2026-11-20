//! Build script for `flowlens-capture`.
//!
//! The `pcap` dependency links against Npcap's import libraries (`wpcap.lib`
//! and `Packet.lib`) on Windows. Those import libraries are vendored in the
//! repository at `wpdlib/`, but the linker only finds them if we emit the
//! corresponding search path. Emitting it here (instead of relying on a
//! machine-specific `.cargo/config.toml`) keeps `cargo test` / `cargo build`
//! working on any checkout.
fn main() {
    // Re-run only when the build script itself changes.
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set by cargo");
    let wpdlib = std::path::Path::new(&manifest_dir).join("../../wpdlib");

    if wpdlib.join("wpcap.lib").exists() {
        println!("cargo:rustc-link-search=native={}", wpdlib.display());
        println!("cargo:rustc-link-lib=wpcap");
        println!("cargo:rustc-link-lib=Packet");
    } else {
        // The SDK is not vendored in this checkout; Npcap may still be
        // discovered through the system library search path.
        println!(
            "cargo:warning=wpdlib/wpcap.lib not found at {}; relying on the system Npcap SDK",
            wpdlib.display()
        );
    }
}
