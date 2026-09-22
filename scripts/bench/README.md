# Per-preset frame-cost bench

Hand-run measurements, not gates: nothing wires these scripts into pre-push or CI. They answer one
question — **what do the ten heaviest presets cost per frame on a named GPU** — so a Linux reading
and a Windows reading of the same laptop can be put side by side. There are two tests, and they
measure different things:

| test | scripts | what it reads | capped by vsync |
|---|---|---|---|
| headless bench | `bench-presets.sh` / `.ps1` | engine cost per frame, no window | no |
| live fullscreen | `live-presets.sh` / `.ps1` | what the operator sees, from `diagnostics.log` | yes |

## The headless bench

Each script runs `ritmolux --stream --sink stdout` once per preset, three times, at 1920x1080 for
1440 frames, and reads the `render+readback N ms` line the stream mode prints at exit
([capturing.md, "What it costs"](../../docs/capturing.md#what-it-costs-and-what-those-numbers-mean)).
That path has **no window, no swapchain and no vsync**, so the number is not capped at the panel's
refresh rate the way the window's `F3` fps is. It includes the GPU-to-CPU readback of every frame (a
roughly constant cost on both OSes) and excludes presenting to a display. **This is the test that
can say which OS is faster**, since the live test saturates at the refresh rate on a fast GPU.

```
./scripts/bench/bench-presets.sh NVIDIA            # Linux (Vulkan)
.\scripts\bench\bench-presets.ps1 -Gpu NVIDIA      # Windows (DX12)
```

The optional second argument (`-Size` on Windows) changes the frame size, e.g. `2560x1440`. About
three minutes. Output: `preset run1 run2 run3 median_ms fps_equiv`.

## The live fullscreen test

Each preset runs in the real window, **fullscreen**, pinned with `--gpu`, for 32 s; the first 6
one-second `diagnostics.log` rows (startup, the fullscreen resize) are dropped and the rest are
summarised. Output: `gpu preset samples fps_med fps_min avg_ms_med p99_ms_med p99_ms_max dropped
samples_under_60`, and each run's `# renderer adapter` line on stderr.

```
./scripts/bench/live-presets.sh [NVIDIA AMD]                # Linux, Hyprland
.\scripts\bench\live-presets.ps1 [-Gpus NVIDIA,AMD]          # Windows
```

- **Linux** opens each window on Hyprland workspace 9, focuses it, fullscreens it with the Lua
  dispatcher (Hyprland 0.56+), and returns to the starting workspace at exit. The workspace must be
  focused: a hidden one gets no frame callbacks and the reading would measure the compositor.
- **Windows** has no workspaces. It sets `[output] fullscreen = true` in
  `%APPDATA%\Ritmolux\config.toml` for the run and restores the file afterwards. Keep the window in
  front and do not touch the machine while it runs — an occluded window throttles.
- Both need a release build and about 11 minutes for the two GPUs.

## Common to both

`--stream` and the window both read live loopback audio. **Play the same track on both machines**
(or nothing on both), or the two readings differ by what the music drove rather than by the OS.

`--gpu NVIDIA` selects by name, not index, because the adapter roster orders differently per OS.
**Without `--gpu` the window takes the surface's default adapter, which on this laptop is the AMD
iGPU** — every unflagged Linux run in the log resolved `RADV RENOIR`. A reading that does not name
its adapter is not comparable (ADR-0071, ADR-0243).

## Memo for an agent running this on Windows

1. Pull, then build release: `cargo build -p standalone --release --bin ritmolux`.
2. `.\target\release\ritmolux.exe --list-adapters` — confirm an NVIDIA `DiscreteGpu` row and an AMD
   `IntegratedGpu` row exist.
3. Ask the owner to start music and leave it playing. Then run, in this order, keeping every
   `renderer` / `# renderer adapter` line:
   - `.\scripts\bench\bench-presets.ps1 -Gpu NVIDIA`
   - `.\scripts\bench\live-presets.ps1` (tell the owner first: it takes the screen for ~11 minutes)
4. Report each table beside the Linux reference below — per preset, Windows vs Linux, and the ratio
   for the headless bench. Say which OS is faster per preset and whether the gap is uniform (a
   backend or driver constant) or concentrated in one family (a shader or pipeline path one backend
   handles worse). For the live test, the NVIDIA rows will likely all sit at the 165 Hz cap; the
   comparison there is p99, the minimum fps and dropped frames, and the AMD rows are the ones that
   can differ in fps.
5. Confirm `config.toml` has `fullscreen = false` again afterwards (the script restores it in a
   `finally`, but a killed shell skips that).
6. Save the raw output under `results/` beside the Linux files, named the same way with `windows`
   in place of `linux` and today's date (`windows-<date>-bench-nvidia.tsv`,
   `windows-<date>-live.tsv`), with the same `#` header lines: OS build, commit, adapter and driver,
   size, whether music played. Stage those files by explicit path and commit them; do not write the
   reading into `docs/` on your own — hand the comparison back to the owner. **Neither `.ps1` had
   been run when it was written**; if one misbehaves, fix it here and say what you changed.

## Saved results

`results/` holds every reading as tab-separated text, one file per test per OS per date, each opening
with `#` lines that name the machine, build, adapter, driver and conditions:

| file | what |
|---|---|
| `linux-2026-09-22-bench-nvidia.tsv` | headless bench, the ten presets, 3 runs each, dGPU |
| `linux-2026-09-22-live.tsv` | live fullscreen, the ten presets, both GPUs |
| `linux-2026-09-22-sweep-nvidia.tsv` | headless, the whole shipped library, one run each, dGPU |
| `linux-2026-09-22-sweep-amd.tsv` | the same on the iGPU — the sweep that picked the ten |

A later reading is a new file with a new date, never an edit of an old one: a comparison needs both
ends to stay put.

## Linux reference reading, 2026-09-22

RTX 3080 Laptop GPU, Vulkan, driver NVIDIA 610.57.04, and AMD Radeon (RADV RENOIR, Mesa 26.2.2);
Arch, kernel 7.2.5, Hyprland 0.56.2, build `144b137c`. The panel is the internal 2560x1440 at 165 Hz,
wired to the iGPU, so a dGPU frame is copied across (PRIME) before it is shown.

### Headless bench, 1920x1080, no music

The iGPU column is a single 720-frame run per preset, kept because it is what picked this set:
these are the ten slowest of the shipped library on the iGPU. On the dGPU the heaviest presets are
the swarms, not these attractors; the set is chosen for where lag was seen.

| preset | dGPU median ms | iGPU ms |
|---|---|---|
| Nebula | 4.12 | 31.85 |
| Leviathan | 4.51 | 30.43 |
| Clifford | 3.85 | 23.53 |
| Volute | 3.91 | 19.71 |
| Dragon | 3.90 | 19.39 |
| Ink on Paper | 3.31 | 18.01 |
| Barnsley Fern | 3.82 | 17.23 |
| Braid | 5.80 | 14.78 |
| Murmuration | 5.90 | 13.10 |
| Shatter | 5.27 | 11.21 |

### Live fullscreen, 2560x1440, music playing

25 one-second samples per row; capture `live PulseAudio 48000/2` throughout; zero dropped frames in
every row.

| preset | NVIDIA fps med / min | NVIDIA p99 med / max | AMD fps med / min | AMD p99 med / max | AMD samples < 60 |
|---|---|---|---|---|---|
| Nebula | 164.9 / 164.8 | 6.60 / 6.92 | 25.3 / 25.1 | 43.79 / 68.19 | 25 |
| Leviathan | 164.9 / 164.9 | 6.49 / 7.17 | 25.4 / 25.1 | 49.38 / 82.47 | 25 |
| Clifford | 164.9 / 164.9 | 6.50 / 6.71 | 31.2 / 30.1 | 38.12 / 64.08 | 25 |
| Volute | 164.9 / 164.9 | 6.47 / 6.69 | 46.1 / 36.5 | 28.16 / 33.46 | 20 |
| Dragon | 164.9 / 164.9 | 6.45 / 6.62 | 44.1 / 41.8 | 30.96 / 32.21 | 25 |
| Ink on Paper | 164.9 / 164.9 | 6.30 / 6.36 | 36.8 / 33.7 | 36.58 / 62.62 | 25 |
| Barnsley Fern | 164.9 / 164.9 | 6.45 / 6.73 | 50.4 / 44.0 | 25.61 / 31.47 | 25 |
| Braid | 164.9 / 164.8 | 6.72 / 6.89 | 77.6 / 71.4 | 14.70 / 15.49 | 0 |
| Murmuration | 164.9 / 164.8 | 6.75 / 7.07 | 86.9 / 83.7 | 13.49 / 14.22 | 0 |
| Shatter | 164.9 / 164.8 | 6.63 / 6.74 | 102.0 / 88.1 | 13.48 / 15.03 | 0 |

The dGPU holds the 165 Hz cap on all ten, matching the Windows dGPU reference in `docs/nfr.md`
(165.0 fps median, p99 6.3-8.9 ms). The iGPU misses 60 fps on the seven attractor-heavy presets.
