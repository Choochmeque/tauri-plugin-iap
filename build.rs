#[cfg(target_os = "macos")]
use std::{path::PathBuf, process::Command};

const COMMANDS: &[&str] = &[
    "register_listener",
    "remove_listener",
    "get_products",
    "purchase",
    "restore_purchases",
    "get_purchase_history",
    "acknowledge_purchase",
    "consume_purchase",
    "get_product_status",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();

    #[cfg(target_os = "macos")]
    {
        // Only run macOS-specific build steps when building for macOS
        if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "macos" {
            // Rebuild when target architecture or deployment target changes
            println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");
            println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");

            let bridges = vec!["src/macos.rs"];
            for path in &bridges {
                println!("cargo:rerun-if-changed={path}");
            }

            println!("cargo:rerun-if-changed=macos/Sources/IapPlugin.swift");

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

#[cfg(target_os = "macos")]
fn compile_swift() {
    let swift_package_dir = manifest_dir().join("macos");
    let target_triple = swift_target_triple();

    let mut cmd = Command::new("swift");

    cmd.current_dir(&swift_package_dir)
        .arg("build")
        // Build into OUT_DIR (under target/) instead of the default `.build`
        // inside the crate source. Source-tree writes don't survive a clean
        // registry re-extraction / cache restore, which leaves cargo's
        // fingerprint saying "built" while the linked artifact is gone.
        .args([
            "--scratch-path",
            swift_build_dir()
                .to_str()
                .expect("Swift build path must be valid UTF-8"),
        ])
        .args(["--triple", &target_triple])
        .args([
            "-Xswiftc",
            "-import-objc-header",
            "-Xswiftc",
            swift_source_dir()
                .join("bridging-header.h")
                .to_str()
                .expect("Bridging header path must be valid UTF-8"),
        ]);

    if is_release_build() {
        cmd.args(["-c", "release", "-Xswiftc", "-enable-testing"]);
    }

    let exit_status = cmd
        .spawn()
        .expect("Failed to spawn swift build command")
        .wait_with_output()
        .expect("Failed to wait for swift build output");

    assert!(
        exit_status.status.success(),
        r"
Swift build failed for target: {}
Stderr: {}
Stdout: {}
",
        target_triple,
        String::from_utf8(exit_status.stderr).expect("Stderr must be valid UTF-8"),
        String::from_utf8(exit_status.stdout).expect("Stdout must be valid UTF-8"),
    );
}

#[cfg(target_os = "macos")]
fn swift_bridge_out_dir() -> PathBuf {
    generated_code_dir()
}

#[cfg(target_os = "macos")]
fn manifest_dir() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");
    PathBuf::from(manifest_dir)
}

#[cfg(target_os = "macos")]
fn out_dir() -> PathBuf {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set");
    PathBuf::from(out_dir)
}

/// `SwiftPM` scratch (build) directory, under `OUT_DIR` so it lives in `target/`
/// and is covered by cargo's fingerprint and any build cache.
#[cfg(target_os = "macos")]
fn swift_build_dir() -> PathBuf {
    out_dir().join("swift-build")
}

#[cfg(target_os = "macos")]
fn is_release_build() -> bool {
    std::env::var("PROFILE").expect("PROFILE must be set") == "release"
}

#[cfg(target_os = "macos")]
fn swift_source_dir() -> PathBuf {
    manifest_dir().join("macos/Sources")
}

#[cfg(target_os = "macos")]
fn generated_code_dir() -> PathBuf {
    swift_source_dir().join("generated")
}

#[cfg(target_os = "macos")]
fn target_arch() -> String {
    std::env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH must be set")
}

#[cfg(target_os = "macos")]
fn swift_arch() -> &'static str {
    match target_arch().as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        arch => panic!("Unsupported architecture for macOS: {arch}"),
    }
}

#[cfg(target_os = "macos")]
fn macos_deployment_target() -> String {
    std::env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "13.0".to_string())
}

#[cfg(target_os = "macos")]
fn swift_target_triple() -> String {
    format!("{}-apple-macosx{}", swift_arch(), macos_deployment_target())
}

#[cfg(target_os = "macos")]
fn swift_bin_path() -> Option<PathBuf> {
    let mut cmd = Command::new("swift");
    cmd.current_dir(manifest_dir().join("macos"))
        .arg("build")
        .args([
            "--scratch-path",
            swift_build_dir()
                .to_str()
                .expect("Swift build path must be valid UTF-8"),
        ])
        .args(["--triple", &swift_target_triple()])
        .arg("--show-bin-path");

    if is_release_build() {
        cmd.args(["-c", "release"]);
    }

    let output = cmd.output().ok()?;
    if output.status.success() {
        let path_str = String::from_utf8(output.stdout).ok()?;
        let path = PathBuf::from(path_str.trim());
        if path.join("libtauri-plugin-iap.a").exists() {
            return Some(path);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn swift_library_static_lib_dir() -> PathBuf {
    // 1. Resolve directly from the current SwiftPM invocation
    if let Some(bin_path) = swift_bin_path() {
        return bin_path;
    }

    let debug_or_release = if is_release_build() {
        "release"
    } else {
        "debug"
    };

    let products_subdir = if is_release_build() {
        "Release"
    } else {
        "Debug"
    };

    let arch_dir = format!("{}-apple-macosx", swift_arch());

    // 2. Prioritize modern SwiftPM layouts over legacy layouts
    let candidates = [
        swift_build_dir().join("out/Products").join(products_subdir),
        swift_build_dir().join(debug_or_release),
        swift_build_dir().join(format!("{arch_dir}/{debug_or_release}")),
    ];

    // 3. If multiple candidates exist, select the newest artifact to prevent stale caches
    let mut newest_candidate: Option<(PathBuf, std::time::SystemTime)> = None;

    for candidate in &candidates {
        let lib_path = candidate.join("libtauri-plugin-iap.a");
        if let Ok(metadata) = std::fs::metadata(&lib_path) {
            let mtime = metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            match &newest_candidate {
                None => newest_candidate = Some((candidate.clone(), mtime)),
                Some((_, best_mtime)) if mtime > *best_mtime => {
                    newest_candidate = Some((candidate.clone(), mtime));
                }
                _ => {}
            }
        }
    }

    if let Some((best_path, _)) = newest_candidate {
        return best_path;
    }

    candidates[0].clone()
}
