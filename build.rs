#[cfg(all(feature = "unstable", target_os = "macos"))]
use std::{path::PathBuf, process::Command};

const COMMANDS: &[&str] = &[
    "initialize",
    "get_products",
    "purchase",
    "restore_purchases",
    "get_purchase_history",
    "acknowledge_purchase",
    "get_product_status",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();

    #[cfg(all(feature = "unstable", target_os = "macos"))]
    {
        // Only run macOS-specific build steps when building for macOS
        if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "macos" {
            let bridges = vec!["src/macos.rs"];
            for path in &bridges {
                println!("cargo:rerun-if-changed={path}");
            }

            swift_bridge_build::parse_bridges(bridges)
                .write_all_concatenated(swift_bridge_out_dir(), env!("CARGO_PKG_NAME"));

            compile_swift();

            println!("cargo:rustc-link-lib=static=tauri-plugin-iap");
            println!(
                "cargo:rustc-link-search={}",
                swift_library_static_lib_dir()
                    .to_str()
                    .expect("Swift library path must be valid UTF-8")
            );
        }
    }
}

#[cfg(all(feature = "unstable", target_os = "macos"))]
fn compile_swift() {
    let swift_package_dir = manifest_dir().join("macos");

    let mut cmd = Command::new("swift");

    cmd.current_dir(swift_package_dir).arg("build").args([
        "-Xswiftc",
        "-import-objc-header",
        "-Xswiftc",
        swift_source_dir()
            .join("bridging-header.h")
            .to_str()
            .expect("Bridging header path must be valid UTF-8"),
    ]);

    if is_release_build() {
        cmd.args(["-c", "release"]);
    }

    let exit_status = cmd
        .spawn()
        .expect("Failed to spawn swift build command")
        .wait_with_output()
        .expect("Failed to wait for swift build output");

    if !exit_status.status.success() {
        panic!(
            r#"
Stderr: {}
Stdout: {}
"#,
            String::from_utf8(exit_status.stderr).expect("Stderr must be valid UTF-8"),
            String::from_utf8(exit_status.stdout).expect("Stdout must be valid UTF-8"),
        )
    }
}

#[cfg(all(feature = "unstable", target_os = "macos"))]
fn swift_bridge_out_dir() -> PathBuf {
    generated_code_dir()
}

#[cfg(all(feature = "unstable", target_os = "macos"))]
fn manifest_dir() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");
    PathBuf::from(manifest_dir)
}

#[cfg(all(feature = "unstable", target_os = "macos"))]
fn is_release_build() -> bool {
    std::env::var("PROFILE").expect("PROFILE must be set") == "release"
}

#[cfg(all(feature = "unstable", target_os = "macos"))]
fn swift_source_dir() -> PathBuf {
    manifest_dir().join("macos/Sources")
}

#[cfg(all(feature = "unstable", target_os = "macos"))]
fn generated_code_dir() -> PathBuf {
    swift_source_dir().join("generated")
}

#[cfg(all(feature = "unstable", target_os = "macos"))]
fn swift_library_static_lib_dir() -> PathBuf {
    let debug_or_release = if is_release_build() {
        "release"
    } else {
        "debug"
    };

    manifest_dir().join(format!("macos/.build/{debug_or_release}"))
}
