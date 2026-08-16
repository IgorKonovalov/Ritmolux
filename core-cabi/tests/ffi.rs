//! First automated coverage of the C ABI (the long-standing zero-CI-coverage
//! gap noted in the Plan 0001/0002 reviews). Drives lmv_create ->
//! lmv_load_presets -> lmv_free across the FFI boundary against a temp dir,
//! confirms the v2 version handshake, and exercises the null-path error path
//! (no UB, documented negative code). No window is attached, so this runs
//! headless: lmv_load_presets stashes the loaded set as pending until a
//! renderer exists, and still reports the loaded count.

use std::path::Path;

use lmv_core_c::{
    LMV_ABI_VERSION, LMV_DEBUG_OVERLAY, LMV_ERR_INVALID_ARG, LMV_ERR_NO_WINDOW, LMV_OK, LmvMetrics,
    lmv_abi_version, lmv_create, lmv_free, lmv_get_metrics, lmv_load_presets, lmv_render,
    lmv_render_dt, lmv_set_debug, lmv_set_now_playing,
};

/// Count the `.toml` files in `dir` (0 if it can't be read).
fn toml_count(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
                .count()
        })
        .unwrap_or(0)
}

#[test]
fn load_presets_seeds_and_installs_over_the_abi() {
    let dir = std::env::temp_dir().join("lmv_ffi_load_presets_test");
    let _ = std::fs::remove_dir_all(&dir);

    let handle = lmv_create(48_000, 2);
    assert!(
        !handle.is_null(),
        "lmv_create returns a handle for a valid format"
    );

    // Loading against a fresh dir seeds the curated set and installs every
    // valid preset; the return is that count.
    let path = dir.to_str().expect("temp path is valid UTF-8");
    let bytes = path.as_bytes();
    let installed = unsafe { lmv_load_presets(handle, bytes.as_ptr(), bytes.len()) };

    let expected = lmv_core::preset::default_presets().len() as i32;
    assert!(installed > 0, "at least one curated preset installs");
    assert_eq!(
        installed, expected,
        "every embedded curated preset loads over the ABI"
    );
    assert_eq!(
        toml_count(&dir) as i32,
        expected,
        "the temp dir was seeded with the curated files"
    );

    // A null path is rejected with the documented error and no UB.
    let err = unsafe { lmv_load_presets(handle, std::ptr::null(), 0) };
    assert_eq!(err, LMV_ERR_INVALID_ARG, "null path -> invalid arg");

    unsafe { lmv_free(handle) };
    let _ = std::fs::remove_dir_all(&dir);
}

/// Lockstep guard: the Rust `LmvMetrics` must be exactly the 56 bytes the C
/// header's `static_assert(sizeof(LmvMetrics) == 56)` expects. If this breaks,
/// the header's assert would too — fix both together (no cbindgen, ADR-0003).
#[test]
fn lmv_metrics_is_56_bytes() {
    assert_eq!(std::mem::size_of::<LmvMetrics>(), 56);
    assert_eq!(std::mem::align_of::<LmvMetrics>(), 8);
}

#[test]
fn abi_version_is_five() {
    assert_eq!(lmv_abi_version(), 5, "runtime ABI version is v5");
    assert_eq!(LMV_ABI_VERSION, 5, "compile-time ABI version is v5");
}

/// v4 render entry (ADR-0013): `lmv_render_dt` takes a real `dt` and behaves
/// exactly like `lmv_render` for a windowless handle — both drain audio and
/// return `LMV_ERR_NO_WINDOW` (no surface attached here), and both reject a null
/// handle without UB. This guards the added surface + the null path; the actual
/// frame-rate-independent draw is a windowed on-device check (like the plugin's
/// other done-whens).
#[test]
fn render_dt_matches_render_windowless() {
    let handle = lmv_create(48_000, 2);
    assert!(!handle.is_null(), "lmv_create returns a handle");

    // No window attached: both entries report NO_WINDOW, never UB or panic.
    assert_eq!(
        unsafe { lmv_render(handle) },
        LMV_ERR_NO_WINDOW,
        "lmv_render (1/60 wrapper) -> no window"
    );
    assert_eq!(
        unsafe { lmv_render_dt(handle, 1.0 / 144.0) },
        LMV_ERR_NO_WINDOW,
        "lmv_render_dt -> no window"
    );

    // Null handle is the documented invalid-arg error on the new entry too.
    assert_eq!(
        unsafe { lmv_render_dt(std::ptr::null_mut(), 0.016) },
        LMV_ERR_INVALID_ARG,
        "null handle -> invalid arg"
    );

    unsafe { lmv_free(handle) };
}

/// v3 diagnostics ABI (ADR-0008): set the overlay flag and pull a metrics
/// snapshot into a caller-allocated struct, asserting the version + size stamps
/// that guard against a silent Rust/C layout mismatch. Null-arg paths return the
/// documented error with no UB.
///
/// Note: this runs headless (no window), so `lmv_render` returns
/// `LMV_ERR_NO_WINDOW` and the timing fields stay zero — populating them is a
/// windowed runtime check (an attached surface), like the plugin's on-device
/// done-whens. The struct contract this test guards is the silent-memory-bug
/// risk ADR-0008 actually calls out.
#[test]
fn set_debug_and_get_metrics_over_the_abi() {
    let handle = lmv_create(48_000, 2);
    assert!(!handle.is_null(), "lmv_create returns a handle");

    // Toggling the overlay flag is accepted (idempotent, cheap, pre-window).
    let rc = unsafe { lmv_set_debug(handle, LMV_DEBUG_OVERLAY) };
    assert_eq!(rc, LMV_OK, "lmv_set_debug(OVERLAY) -> OK");

    // Pull the snapshot into a caller-allocated, size-declared struct.
    let mut out: LmvMetrics = unsafe { std::mem::zeroed() };
    out.struct_size = std::mem::size_of::<LmvMetrics>() as u32;
    let rc = unsafe { lmv_get_metrics(handle, &mut out) };
    assert_eq!(rc, LMV_OK, "lmv_get_metrics -> OK");
    assert_eq!(out.abi_version, 5, "core stamps the current abi_version");
    assert_eq!(
        out.struct_size,
        std::mem::size_of::<LmvMetrics>() as u32,
        "core stamps the bytes it wrote (full struct here)"
    );

    // Headless renders are no-ops (no window); they must not UB or panic.
    for _ in 0..3 {
        let _ = unsafe { lmv_render(handle) };
    }

    // Null-arg error paths: documented negative code, no UB.
    assert_eq!(
        unsafe { lmv_set_debug(std::ptr::null_mut(), LMV_DEBUG_OVERLAY) },
        LMV_ERR_INVALID_ARG,
        "null handle -> invalid arg"
    );
    assert_eq!(
        unsafe { lmv_get_metrics(handle, std::ptr::null_mut()) },
        LMV_ERR_INVALID_ARG,
        "null out -> invalid arg"
    );

    unsafe { lmv_free(handle) };
}

/// v5's text entry point (ADR-0110). Every rejection the boundary promises,
/// exercised for real: the validate-at-the-boundary rule is only worth stating
/// if the invalid cases are the ones that got tested.
///
/// Headless, so the accepting path lands on `LMV_ERR_NO_WINDOW` rather than
/// `LMV_OK` — which is itself the documented behaviour before a window is
/// attached, and distinguishable from every argument rejection below.
#[test]
fn set_now_playing_validates_at_the_boundary() {
    let handle = lmv_create(48_000, 2);
    assert!(!handle.is_null());

    let track = "Boards of Canada - Roygbiv";
    let bytes = track.as_bytes();

    // Valid UTF-8, valid handle, no window yet: past every argument check.
    assert_eq!(
        unsafe { lmv_set_now_playing(handle, bytes.as_ptr(), bytes.len()) },
        LMV_ERR_NO_WINDOW,
        "a well-formed call reaches the renderer check, not an arg rejection"
    );

    // A null handle.
    assert_eq!(
        unsafe { lmv_set_now_playing(std::ptr::null_mut(), bytes.as_ptr(), bytes.len()) },
        LMV_ERR_INVALID_ARG,
        "null handle -> invalid arg"
    );

    // A null pointer, with a length that would otherwise be read.
    assert_eq!(
        unsafe { lmv_set_now_playing(handle, std::ptr::null(), bytes.len()) },
        LMV_ERR_INVALID_ARG,
        "null text -> invalid arg"
    );

    // A zero length — rejected before the pointer is dereferenced, so this is
    // safe even though the pointer is valid.
    assert_eq!(
        unsafe { lmv_set_now_playing(handle, bytes.as_ptr(), 0) },
        LMV_ERR_INVALID_ARG,
        "zero length -> invalid arg"
    );

    // Invalid UTF-8: a lone continuation byte and a truncated 3-byte sequence.
    // Rejected here rather than trusted inward — the core downstream assumes
    // its string is text.
    for bad in [&[0x80u8][..], &[0xE2, 0x82][..]] {
        assert_eq!(
            unsafe { lmv_set_now_playing(handle, bad.as_ptr(), bad.len()) },
            LMV_ERR_INVALID_ARG,
            "invalid UTF-8 -> invalid arg"
        );
    }

    // Well-formed non-Latin text is *not* rejected — the whole reason ADR-0110
    // chose glyphon over the 31-glyph quad font, which would have painted this
    // blank without reporting anything.
    let bjork = "Björk - Jóga".as_bytes();
    assert_eq!(
        unsafe { lmv_set_now_playing(handle, bjork.as_ptr(), bjork.len()) },
        LMV_ERR_NO_WINDOW,
        "multi-byte UTF-8 passes the boundary check"
    );

    unsafe { lmv_free(handle) };
}

/// The copy-on-receipt rule the spec states and the C++ side must be able to
/// rely on: the core must not retain the caller's pointer. Asserted by freeing
/// the buffer immediately after the call and then driving the handle — a
/// retained pointer would be a use-after-free here, which Miri and the sanitizer
/// builds can see even when a plain run cannot.
#[test]
fn set_now_playing_does_not_retain_the_callers_buffer() {
    let handle = lmv_create(48_000, 2);
    assert!(!handle.is_null());

    {
        let owned = String::from("Sigur Ros - Svefn-g-englar");
        let bytes = owned.as_bytes();
        let _ = unsafe { lmv_set_now_playing(handle, bytes.as_ptr(), bytes.len()) };
        drop(owned); // the caller may free immediately
    }

    // Anything the core does with the string from here on would touch freed
    // memory if it had kept the pointer.
    for _ in 0..3 {
        let _ = unsafe { lmv_render(handle) };
    }

    unsafe { lmv_free(handle) };
}
