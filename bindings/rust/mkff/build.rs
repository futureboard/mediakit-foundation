use std::env;
use std::path::{Path, PathBuf};

/// `mkff-sys`'s build script already emits `cargo:rustc-link-search` /
/// `cargo:rustc-link-lib` for libmkff, and those *do* propagate through
/// to this crate's final test/bin artifacts (Cargo aggregates native
/// link flags across the whole dependency graph). `cargo:rustc-link-arg`
/// does not propagate the same way, though — it only applies to the
/// artifacts of the package whose build script emitted it. Since the
/// rpath needs to land on `mkff`'s own test binaries (so `cargo test`
/// finds libmkff.so.0 without the caller setting LD_LIBRARY_PATH), it
/// has to be emitted again here rather than relied on from mkff-sys.
fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("bindings/rust/mkff is expected to live three directories below the repo root")
        .to_path_buf();

    if let Ok(lib_dir) = env::var("MKFF_LIB_DIR") {
        emit_rpath(Path::new(&lib_dir));
        return;
    }

    if let Some(lib_dir) = find_build_tree_lib_dir(&repo_root) {
        emit_rpath(&lib_dir);
    }
    // If neither is found, mkff-sys's build script will already have
    // panicked with a clear message before we get here.
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

fn emit_rpath(lib_dir: &Path) {
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
}
