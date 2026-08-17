# plugin-foobar

The foobar2000 visualization component: a thin C++ shim over lmv-core's C ABI
(`core-cabi/include/lmv_core.h`). Windows-only per ADR-0001, x64 only.

## SDK location (Plan 0001 Phase 7)

The foobar2000 SDK (release **2025-03-07**) is unpacked at `plugin-foobar/sdk/`:

```
plugin-foobar/sdk/
├── foobar2000/       # SDK proper + component client + helpers
├── pfc/              # foundation classes the SDK depends on
├── libPPUI/          # UI helpers (unused by this component)
└── sdk-license.txt
```

The SDK is third-party and separately licensed — it is gitignored, never
committed (ADR-0115 Alternative A). To stage it:

```powershell
.\packaging\foobar\fetch-sdk.ps1
```

That downloads the **pinned** release against a SHA-256 and unpacks it here. The
pin lives in [`packaging/foobar/sdk-pin.ps1`](../packaging/foobar/sdk-pin.ps1),
which is the one file a bump edits and which explains why the release is pinned
rather than tracked. To do it by hand instead, download from
<https://www.foobar2000.org/SDK> and extract the archive to `plugin-foobar/sdk/`.

Toolchain: MSVC (VS Build Tools 2022, x64).

## What the component's own UI does

Everything the shim offers a user sits on one right-click menu (Plan 0107):
**Preset ▸** (the core's roster, flat, marked on the one showing — selection
goes over C ABI v6, [ADR-0117](../docs/adrs/0117-c-abi-v6-the-host-reads-the-roster-and-selects-a-preset.md)),
**Next scene**, **Reload presets** (re-calls `lmv_load_presets`, then re-selects
the current preset *by name* — `set_presets` keeps the index, not the name),
**Open presets folder**, and the diagnostics-overlay toggle. `Space` is the
keyboard form of Next scene.

The chosen preset is the component's only persisted setting: a `cfg_string`
holding the **name**, restored after the window attaches and the library loads.

## Building

```powershell
.\build.ps1            # -> plugin-foobar\build\foo_lmv.dll
.\build.ps1 -Install   # ...then copy it into the local foobar2000 v2 profile
```

`-Install` writes to `%APPDATA%\foobar2000-v2\user-components-x64\foo_lmv\` —
the development inner loop. It is **not** how a release is produced, and it is
deliberately not what Plan 0102 Phase 5 tests: an artifact that only ever gets
installed over its own build directory has never exercised the path a user takes.

## Packaging a release

```powershell
.\packaging\foobar\build-component.ps1
```

Builds, assembles, stamps the version, packages and **verifies**
`target/dist/light-music-visualizer-v<version>-foobar2000-component.zip`, which
holds `foo_lmv.fb2k-component` and a `READ-ME-FIRST.txt`. Pass `-SkipBuild` to
reuse the DLL already in `build/`.

The verification lives in the script rather than in the release workflow, so a
local run is held to the same bar as CI (ADR-0038's model, applied by ADR-0115).
Every check is fatal, and two of them are why the script exists rather than a
`Compress-Archive` line: the component archive must hold `x64/foo_lmv.dll` and
nothing else — foobar2000 extracts a component's whole archive into the user's
components folder, so a stray file is a real defect — and the DLL must carry the
workspace version rather than `foo_lmv.cpp`'s `0.0.0-dev` fallback.
