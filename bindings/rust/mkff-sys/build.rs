use std::env;
use std::path::{Path, PathBuf};

/// Locates libmkff's shared library (and, for completeness, its public
/// headers) without requiring a `cmake --install` step first: CI and
/// local development both just run `cmake --build` followed by `cargo
/// build`, so this looks directly in the CMake build tree.
///
/// Resolution order:
///   1. `MKFF_LIB_DIR` / `MKFF_INCLUDE_DIR` env vars, if set (explicit
///      override — e.g. pointing at a `cmake --install` prefix).
///   2. `pkg-config --libs mkff`, if a mkff.pc is available.
///   3. The first `<repo_root>/build/*/lib` directory that contains
///      libmkff.so, mirroring CMakePresets.json's binaryDir layout.
fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir
        .parent() // bindings/rust
        .and_then(Path::parent) // bindings
        .and_then(Path::parent) // repo root
        .expect("bindings/rust/mkff-sys is expected to live three directories below the repo root")
        .to_path_buf();

    if let Ok(lib_dir) = env::var("MKFF_LIB_DIR") {
        link(Path::new(&lib_dir));
        return;
    }

    if let Some(lib_dir) = pkg_config_lib_dir() {
        link(&lib_dir);
        return;
    }

    if let Some(lib_dir) = find_build_tree_lib_dir(&repo_root) {
        link(&lib_dir);
        return;
    }

    panic!(
        "could not locate libmkff.so: build the C project first (cmake --build --preset \
         linux-clang-debug), or set MKFF_LIB_DIR to an explicit lib/ directory"
    );
}

fn pkg_config_lib_dir() -> Option<PathBuf> {
    let output = std::process::Command::new("pkg-config")
        .args(["--libs-only-L", "mkff"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let flag = stdout.split_whitespace().find(|s| s.starts_with("-L"))?;
    Some(PathBuf::from(flag.trim_start_matches("-L")))
}

fn find_build_tree_lib_dir(repo_root: &Path) -> Option<PathBuf> {
    let build_dir = repo_root.join("build");
    let entries = std::fs::read_dir(&build_dir).ok()?;
    for entry in entries.flatten() {
        let lib_dir = entry.path().join("lib");
        if lib_dir.join("libmkff.so").exists() {
            return Some(lib_dir);
        }
    }
    None
}

fn link(lib_dir: &Path) {
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=mkff");
    // Embed an rpath so `cargo test`/`cargo run` binaries find
    // libmkff.so at runtime without the caller having to set
    // LD_LIBRARY_PATH — convenient for a build tree that hasn't been
    // installed anywhere yet.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
}
