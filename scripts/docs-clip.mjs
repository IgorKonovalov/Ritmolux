#!/usr/bin/env node
// Render the two moving-picture artifacts: the demo clip and the social preview.
//
// Usage:  node scripts/docs-clip.mjs
// Writes: docs/images/demo.mp4, docs/images/social-preview.png — and a stimulus
//         WAV under target/, which is not committed.
//
// No arguments, no environment, no options — the reason scripts/docs-shots.mjs
// gives, and this is its sibling. THE MANIFEST BELOW IS THE PROVENANCE RECORD:
// the preset, the stimulus, the size, the frame rate and the tier behind each of
// the two files are here and nowhere else (ADR-0100).
//
// Why a sibling and not two more entries in docs-shots.mjs: that script's
// contract is "writes docs/images/**.png and nothing anywhere else", it spawns
// only cargo, and it needs nothing installed beyond the toolchain. A video needs
// an external encoder and writes a container, so folding it in would weaken all
// three claims for the two hundred stills that do not need it.
//
// NOT A CI GATE, for docs-shots.mjs's reason: renders are not byte-reproducible
// across machines, so freshness is a human duty at a named cadence rather than a
// check. Re-run this when what the clip shows changes.
//
// PREREQUISITE: ffmpeg on PATH. None ships with this project — a static build is
// larger than the whole application's size budget (NFR section 4), so `shot`
// streams Y4M into whichever encoder it is pointed at (ADR-0114). Without one,
// this script stops before rendering anything and says so.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";

// ---------------------------------------------------------------------------
// The stimulus
// ---------------------------------------------------------------------------

/// `--render` walks a WAV end to end and REFUSES `--signal`: a synthesized
/// stimulus has no file for the encoder to mux audio from, so the two modes are
/// mutually exclusive by construction. Nothing in this repository writes a WAV,
/// and no clip is committed, so the stimulus is synthesized here.
///
/// It is deliberately the same MUSICAL SHAPE as core's `dynamic_groove`, the
/// `--signal dynamic:110` every committed still is captured under — an eight-beat
/// phrase that builds for six beats and rests for two, over a kick, hats on the
/// eighths and a harmonic pad. It is NOT the same code and does not claim to be
/// sample-identical: this is a second, smaller implementation in a second
/// language, and it exists only because `--render` needs a file.
///
/// Every number a listener would hear is here, so the entry in MANIFEST can name
/// the stimulus by pointing at this block.
const STIMULUS = {
  path: "target/docs-clip/stimulus.wav",
  sampleRate: 48_000,
  channels: 2,
  bpm: 110,
  /// Two eight-beat phrases: build, rest, build, rest. One phrase is 4.4 s at
  /// 110 BPM, which is long enough to see a scene react and too short to see it
  /// recover, so the clip shows the cycle twice.
  beats: 16,
};

/// mulberry32 — a two-line seeded PRNG, so the hats are the same noise on every
/// machine and the clip is reproducible rather than merely similar.
function mulberry32(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/// The interleaved 16-bit PCM body, and the whole synth.
function synthesize() {
  const { sampleRate: sr, channels, bpm, beats } = STIMULUS;
  const beatSecs = 60 / bpm;
  const beatSamples = Math.max(1, Math.round(beatSecs * sr));
  const frames = beatSamples * beats;
  const rng = mulberry32(0x5eed0103);

  const pcm = Buffer.alloc(frames * channels * 2);
  // The kick's frequency changes within the beat, so its phase is INTEGRATED
  // rather than evaluated at t: `sin(TAU * f(t) * t)` sweeps the wrong way.
  let kickPhase = 0;
  let prevWhite = 0;

  for (let i = 0; i < frames; i += 1) {
    const beat = Math.floor(i / beatSamples);
    const within = (i % beatSamples) / beatSamples;
    const since = within * beatSecs;

    // Geometric rather than linear: a ramp that spends half its beats near the
    // top has a mean close to its maximum, and the crest is what makes a band
    // reading move. 0.04 rather than silence keeps the onset detector's floor
    // honest through the rest.
    const slot = beat % 8;
    const phrase = slot >= 6 ? 0.04 : 0.18 * 1.4 ** slot;

    if (i % beatSamples === 0) kickPhase = 0;
    const kickHz = 45 + 60 * Math.exp(-since * 45);
    kickPhase += (2 * Math.PI * kickHz) / sr;
    const kick = Math.sin(kickPhase) * Math.exp(-since * 26) * 0.45;

    // Differenced white noise is a one-tap high-pass. Flat white spends most of
    // its amplitude below 4 kHz where the kick and pad already are, so an
    // un-brightened tick costs headroom to light a band it barely reaches.
    const white = rng() * 2 - 1;
    const tick = white - prevWhite;
    prevWhite = white;
    const eighth = beatSecs * 0.5;
    const hatT = since - eighth * Math.floor(since / eighth);
    const hat = tick * Math.exp(-hatT * 16) * (within >= 0.5 ? 2.6 : 1.6);

    // Two voices a fifth apart, five harmonics at 1/k each. The harmonics are
    // the point: the mid band is a MEAN over ~250 Hz-4 kHz, so a bare chord at
    // 165-250 lands in bass and reads as a trickle in mid. The per-harmonic
    // phase offset costs nothing and stops every partial aligning once per
    // period, which would otherwise set the whole signal's peak.
    const t = i / sr;
    let voices = 0;
    for (const f0 of [165, 247.5]) {
      for (let k = 1; k <= 5; k += 1) {
        voices += Math.sin(2 * Math.PI * f0 * k * t + k * 1.7) / k;
      }
    }
    const pad = voices * 0.4 * (0.35 + 0.65 * (1 - Math.exp(-since * 6)));

    // Soft-clipped rather than peak-normalized: dividing by the loudest sample
    // makes the three layers zero-sum, so no setting lights all three bands. The
    // phrase multiplies BEFORE the saturator, so the rest stays in its linear
    // region and the dynamics survive.
    const mono = Math.tanh((kick + hat + pad) * phrase * 1.2) * 0.9;
    const s = Math.max(-32768, Math.min(32767, Math.round(mono * 32767)));
    for (let c = 0; c < channels; c += 1) {
      pcm.writeInt16LE(s, (i * channels + c) * 2);
    }
  }
  return pcm;
}

/// A canonical 44-byte-header PCM WAV — what `shot`'s reader accepts (format
/// tag 1, 16-bit).
function writeWav(path, pcm) {
  const { sampleRate, channels } = STIMULUS;
  const byteRate = sampleRate * channels * 2;
  const header = Buffer.alloc(44);
  header.write("RIFF", 0);
  header.writeUInt32LE(36 + pcm.length, 4);
  header.write("WAVE", 8);
  header.write("fmt ", 12);
  header.writeUInt32LE(16, 16); // fmt chunk size
  header.writeUInt16LE(1, 20); // PCM
  header.writeUInt16LE(channels, 22);
  header.writeUInt32LE(sampleRate, 24);
  header.writeUInt32LE(byteRate, 28);
  header.writeUInt16LE(channels * 2, 32); // block align
  header.writeUInt16LE(16, 34); // bits per sample
  header.write("data", 36);
  header.writeUInt32LE(pcm.length, 40);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, Buffer.concat([header, pcm]));
}

// ---------------------------------------------------------------------------
// The manifest
// ---------------------------------------------------------------------------

/// Both entries render `fragment_tunnel`, the preset docs/images/hero.png is
/// taken from. That is the choice, not an accident: the clip and the preview are
/// what a stranger meets BEFORE the README, and meeting a different picture in
/// each place would read as three projects.
const MANIFEST = [
  {
    kind: "clip",
    out: "docs/images/demo.mp4",
    presetFile: "presets/fragment_tunnel.toml",
    size: "1280x720",
    tier: "rich",
    fps: 30,
    // 28 rather than `shot`'s archival default of 18. A capture is evidence
    // first, which is why the default does not move — but this one is committed
    // and then uploaded to a forum post, so its size is paid by every clone
    // forever. Measured on this preset at this size: 10.7 MB at crf 23 against
    // 6.2 MB at 28 for the same 8.7 s, with the colour tags untouched either way
    // (docs/capturing.md). A field of fine filaments is the content h.264 gives
    // back least on, which is why the saving is well short of the halving the
    // lever manages on flatter material.
    crf: 28,
  },
  {
    kind: "still",
    out: "docs/images/social-preview.png",
    presetFile: "presets/fragment_tunnel.toml",
    // GitHub's social preview box. 1280x640 is its 2:1 slot, so a wider crop of
    // the hero rather than a scaled copy of it — anything else is letterboxed
    // grey in the chat card the picture exists for.
    size: "1280x640",
    tier: "rich",
    // The last hop of the loudest beat, the hop every committed still is taken
    // at. 48 kHz / 512-sample hops is 93.75 hops per second, so 300 is 3.2 s in:
    // the top of the first phrase's build, just before the two-beat rest.
    hop: 300,
  },
];

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

/// The one directory this script is allowed to write a committed file into. An
/// entry pointing anywhere else is a bug, and one that overwrote a source file
/// would be an expensive one to notice.
const IMAGE_ROOT = resolve("docs/images");

for (const [index, entry] of MANIFEST.entries()) {
  const where = `manifest entry ${index} (${entry.out})`;
  const out = resolve(entry.out);
  if (!out.startsWith(IMAGE_ROOT + sep)) {
    throw new Error(`${where}: writes outside docs/images/ (${relative(".", out)}) — refusing`);
  }
  if (!existsSync(entry.presetFile)) {
    throw new Error(`${where}: no preset file at ${entry.presetFile}`);
  }
}

// Checked before anything is rendered: the clip's encoder is external and the
// only failure mode worth spending four minutes of GPU time to discover is not
// this one.
try {
  execFileSync("ffmpeg", ["-version"], { stdio: "ignore" });
} catch {
  console.error(
    "ffmpeg is not on PATH, and none ships with this project (ADR-0114).\n" +
      "Install one and re-run: the clip is streamed into it and the audio is muxed from the stimulus.",
  );
  process.exit(1);
}

writeWav(STIMULUS.path, synthesize());
console.log(
  `${STIMULUS.path}\n  ${STIMULUS.beats} beats at ${STIMULUS.bpm} BPM, ` +
    `${STIMULUS.sampleRate} Hz x${STIMULUS.channels}`,
);

let failures = 0;
for (const entry of MANIFEST) {
  const args = [
    "run", "--release", "-p", "standalone", "--example", "shot", "--",
    "--preset-file", entry.presetFile,
    "--size", entry.size,
    "--tier", entry.tier,
    "--out", entry.out,
  ];
  if (entry.kind === "clip") {
    args.push("--render", STIMULUS.path, "--fps", String(entry.fps));
    args.push("--ffmpeg", "ffmpeg", "--crf", String(entry.crf));
    console.log(
      `\n${entry.out}\n  ${entry.presetFile} rendered over ${STIMULUS.path}, ` +
        `${entry.fps} fps, ${entry.size}, tier ${entry.tier}, crf ${entry.crf}`,
    );
  } else {
    args.push("--audio", STIMULUS.path, "--frame-at", String(entry.hop));
    console.log(
      `\n${entry.out}\n  ${entry.presetFile} @ hop ${entry.hop} of ${STIMULUS.path}, ` +
        `${entry.size}, tier ${entry.tier}`,
    );
  }
  try {
    execFileSync("cargo", args, { stdio: "inherit" });
  } catch (err) {
    // A non-zero `shot` must not leave the previous file sitting there looking
    // current: `git status` would be clean over a stale artifact and the run's
    // whole freshness claim would be false.
    rmSync(entry.out, { force: true });
    console.error(`FAILED: ${entry.out} (previous file removed): ${err.message}`);
    failures += 1;
  }
}

if (failures > 0) {
  console.error(`\n${failures} of ${MANIFEST.length} artifacts failed to render`);
  process.exit(1);
}
console.log(`\n${MANIFEST.length} artifacts written under docs/images/`);
