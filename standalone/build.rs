//! Compile the Spout C++ shim, and only when asked (Plan 0115 Phase 3).
//!
//! **This script does nothing at all unless the `spout` feature is on and the
//! target is Windows.** That is the property that keeps every ordinary build
//! untouched: without the feature there is no C++ compiler step, no SDK, and no
//! network — `cargo build`, `cargo clippy` and `cargo nextest run` behave
//! exactly as they did before this file existed, on every platform.
//!
//! Features reach a build script as `CARGO_FEATURE_<NAME>` in the environment,
//! not as `cfg`, because the script is compiled for the host rather than for
//! the target; the same goes for the target OS, which is `CARGO_CFG_TARGET_OS`.
//! Reading `cfg!(feature = "spout")` here would silently be the host's answer.

use std::env;
use std::path::{Path, PathBuf};

/// Where `packaging/spout/fetch-sdk.ps1` leaves the staged SDK, relative to
/// this crate. The script flattens the archive's `Libs_<version>/` directory
/// into this root, which is what keeps the paths below free of the version.
const SDK_DIR: &str = "spout-sdk";

fn main() {
    println!("cargo:rerun-if-changed=src/spout/shim.cpp");
    println!("cargo:rerun-if-changed=build.rs");

    if env::var_os("CARGO_FEATURE_SPOUT").is_none() {
        return;
    }
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        panic!(
            "the `spout` feature is Windows-only (target_os = {target_os}). Spout has no macOS \
             form; the analogue there is Syphon, a different SDK against a Metal/IOSurface seam \
             (ADR-0125). Build without --features spout."
        );
    }

    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets this"));
    let sdk = crate_dir.join(SDK_DIR);
    let include = sdk.join("include").join("SpoutDX");
    // MD, not MT: rustc's MSVC target links the dynamic CRT, and mixing the two
    // is a duplicate-symbol link failure rather than a runtime surprise.
    let lib_dir = sdk.join("MD").join("lib");
    require_staged(&include.join("SpoutDX.h"), &sdk);
    require_staged(&lib_dir.join("SpoutDX_static.lib"), &sdk);

    // `SPOUT_DLLEXP` expands to nothing unless SPOUT_BUILD_DLL or
    // SPOUT_IMPORT_DLL is defined, which is what makes the static library the
    // no-configuration path: defining neither is correct here.
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .file("src/spout/shim.cpp")
        .include(&include)
        .compile("rlx_spout_shim");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=SpoutDX_static");
    // What SpoutDX itself calls into: the D3D11 device and DXGI adapter
    // enumeration, plus the Win32 surface SpoutUtils uses for its registry
    // settings and its SpoutPanel launch.
    for system_lib in [
        "d3d11", "dxgi", "shell32", "advapi32", "user32", "gdi32", "ole32",
    ] {
        println!("cargo:rustc-link-lib=dylib={system_lib}");
    }
}

/// Fail with an actionable message rather than a C++ include error when the SDK
/// has not been staged.
fn require_staged(path: &Path, sdk: &Path) {
    if path.exists() {
        return;
    }
    panic!(
        "the `spout` feature needs the Spout SDK staged at {}, and {} is missing.\n\
         Run:  powershell -File packaging/spout/fetch-sdk.ps1\n\
         The SDK is third-party, pinned by hash and never committed (ADR-0125).",
        sdk.display(),
        path.display()
    );
}
