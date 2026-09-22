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
   reading into `docs/` on your own — hand the comparison back to the owner. **Both `.ps1` scripts
   were first run on 2026-09-22** and worked as written but for the number format: a comma-decimal
   locale printed `5,68` into the tab-separated output, so each now sets the invariant culture
   before printing. If one misbehaves again, fix it here and say what you changed.

## Saved results

`results/` holds every reading as tab-separated text, one file per test per OS per date, each opening
with `#` lines that name the machine, build, adapter, driver and conditions:

| file | what |
|---|---|
| `linux-2026-09-22-bench-nvidia.tsv` | headless bench, the ten presets, 3 runs each, dGPU |
| `linux-2026-09-22-live.tsv` | live fullscreen, the ten presets, both GPUs |
| `linux-2026-09-22-sweep-nvidia.tsv` | headless, the whole shipped library, one run each, dGPU |
| `linux-2026-09-22-sweep-amd.tsv` | the same on the iGPU — the sweep that picked the ten |
| `windows-2026-09-22-bench-nvidia.tsv` | headless bench, the ten presets, 3 runs each, dGPU |
| `windows-2026-09-22-live.tsv` | live fullscreen, the ten presets, both GPUs |

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

## Windows reading, 2026-09-22

The same laptop, the same ten presets, build `ff474fee`: RTX 3080 Laptop GPU and AMD Radeon(TM)
Graphics, both on DX12, drivers 32.0.15.8142 and 30.0.13002.1001; Windows 10 Home 22H2
(10.0.19045.6466), the internal 2560x1440 165 Hz panel. Files:
`windows-2026-09-22-bench-nvidia.tsv`, `windows-2026-09-22-live.tsv`.

**Two things weaken the comparison, and neither is fixable after the fact.** The headless bench ran
**with music playing** where the Linux one had none, so a preset whose point count follows the audio
is not compared like for like there; the live test had music on both machines. And each Windows live
row kept 22 one-second samples against Linux's 25 — startup takes longer here, so more of the fixed
32 s was spent before the log steadied. Medians are unaffected by the sample count.

### Headless bench, 1920x1080, dGPU, music playing

No iGPU column: the sweep that picked the ten was not repeated here, only the ten-preset bench.

| preset | Windows DX12 ms | Linux Vulkan ms | Win/Linux |
|---|---|---|---|
| Nebula | 5.68 | 4.12 | 1.38 |
| Leviathan | 5.76 | 4.51 | 1.28 |
| Clifford | 5.46 | 3.85 | 1.42 |
| Volute | 5.10 | 3.91 | 1.30 |
| Dragon | 5.11 | 3.90 | 1.31 |
| Ink on Paper | 4.73 | 3.31 | 1.43 |
| Barnsley Fern | 5.03 | 3.82 | 1.32 |
| Braid | 7.40 | 5.80 | 1.28 |
| Murmuration | 7.46 | 5.90 | 1.26 |
| Shatter | 6.76 | 5.27 | 1.28 |

Linux is ahead on every preset, and the gap is an **additive 1.2-1.6 ms per frame** rather than a
ratio that tracks the preset's weight - Ink on Paper, the cheapest, pays the same absolute penalty
as Murmuration, the dearest. That shape points at a fixed per-frame cost on this path (submission,
the readback, the driver), not at one shader family DX12 compiles worse. The three runs per preset
agreed within 0.1 ms.

### Live fullscreen, 2560x1440, music playing

22 one-second samples per row; capture `live WASAPI 48000/2` throughout; zero dropped frames in
every row.

| preset | NVIDIA fps med / min | NVIDIA p99 med / max | AMD fps med / min | AMD p99 med / max | AMD samples < 60 |
|---|---|---|---|---|---|
| Nebula | 165.0 / 164.9 | 7.54 / 7.89 | 26.4 / 26.2 | 49.83 / 53.01 | 22 |
| Leviathan | 165.0 / 164.7 | 7.52 / 7.94 | 27.2 / 26.4 | 48.69 / 50.55 | 22 |
| Clifford | 165.0 / 164.9 | 7.23 / 7.78 | 33.2 / 31.5 | 40.79 / 41.28 | 22 |
| Volute | 165.0 / 164.9 | 7.09 / 7.29 | 60.7 / 40.3 | 32.40 / 37.64 | 11 |
| Dragon | 165.0 / 164.9 | 7.11 / 7.43 | 47.0 / 44.7 | 30.96 / 33.13 | 22 |
| Ink on Paper | 165.0 / 165.0 | 6.74 / 7.34 | 42.5 / 39.9 | 35.87 / 37.66 | 22 |
| Barnsley Fern | 165.0 / 165.0 | 7.04 / 7.54 | 52.1 / 49.4 | 27.43 / 30.64 | 16 |
| Braid | 165.0 / 165.0 | 6.91 / 7.29 | 86.1 / 81.4 | 14.71 / 15.29 | 0 |
| Murmuration | 165.0 / 164.9 | 7.01 / 7.63 | 93.9 / 89.1 | 13.88 / 16.14 | 0 |
| Shatter | 165.0 / 165.0 | 6.93 / 7.17 | 107.1 / 99.7 | 15.96 / 16.98 | 0 |

**The dGPU result agrees with Linux and with `docs/nfr.md`**: the cap is held on all ten, nothing
drops, and p99 stays under 8 ms. Windows sits 0.5-0.9 ms higher at p99 than Linux does, which is the
same additive penalty the headless bench measures, spent inside a frame budget of 6.06 ms that has
room for it.

**The iGPU result reverses the headless verdict.** Windows is faster in every row - by 3-15%, and by
32% on Volute, which is also the row that swings most (median 60.7 fps, minimum 40.3). Linux's worst
frames are much worse: on the four heaviest presets its p99 max reaches 64-82 ms where Windows stays
at 41-53 ms. So the OS that loses by a constant on the dGPU wins on the iGPU, which is where the
laptop's panel is wired and where the lag that prompted this bench was seen. The same seven presets
miss 60 fps on both.
