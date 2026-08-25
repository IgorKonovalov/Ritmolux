#!/usr/bin/env python3
"""Plan 0106 Phases 3 and 4 done-when, as an executable check.

Runs with the standard library only and needs no GPU, no weights and no model -
which is the property that makes this stage, alone in this feature, gateable.

    python tools/sd-filter/test_sd_filter.py

The end-to-end half needs a built `shot` and nothing else - it synthesizes its
own WAV from the standard library, so it RUNS on any checkout rather than
depending on a file that exists on one machine. Without a built `shot` it SKIPS
with a printed notice rather than passing quietly (ADR-0016), because a check
that reports success when it did not run is worse than one that is absent.

One group has a dependency: the colour-table pin needs `numpy`, because the
conversions it pins are array functions. It skips with the same notice when
numpy is absent, and CI installs numpy so that it does not.

What is NOT here, deliberately: any assertion about what the model draws. That
output is not reproducible across machines - fp16 reduction order and cuDNN
autotuning see to it - so per ADR-0121 no diffused frame may become a baseline.
The properties below are the ones that survive that: the frame count, the
geometry arithmetic, and the reproducibility of a configuration from its echo.
"""

import io
import math
import os
import shlex
import struct
import subprocess
import sys
import tempfile
import wave

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)

import sd_filter  # noqa: E402

FAILURES = []
NL = bytes([10])  # written this way so the escape survives the shell heredoc


def check(name, cond, detail=""):
    print("  %-52s %s" % (name, "ok" if cond else "FAIL"))
    if not cond:
        FAILURES.append("%s %s" % (name, detail))


def synth(width, height, frames, colour=b"444"):
    """A Y4M stream in `shot`'s exact shape, with payload bytes chosen to be hostile."""
    ratio = sd_filter.PLANE_RATIO[colour]
    n = int(width * height * ratio)
    out = io.BytesIO()
    out.write(
        b"YUV4MPEG2 W%d H%d F30:1 Ip A1:1 C%s XCOLORRANGE=FULL\n"
        % (width, height, colour)
    )
    for i in range(frames):
        out.write(b"FRAME\n")
        # 0x0A and 0x0D on purpose: a text-mode handle mangles exactly these,
        # and a payload of zeroes would not notice.
        out.write(bytes(((i * 7 + j) % 256) for j in range(n)))
    return out.getvalue()


def synth_wav(path, seconds=0.5, rate=48000, channels=2):
    """A valid 16-bit PCM WAV, written from the standard library alone.

    This group used to read `spike/clip.wav` - untracked, unignored, and present
    on exactly one machine. With a built `shot` and no spike directory, which is
    the only configuration anyone else has, `shot` exited non-zero and the group
    FAILED rather than skipped. The property under test is byte identity through
    the filter, not the picture, so any valid audio does; synthesizing it is what
    lets this group run everywhere instead of adding a second skip.
    """
    n = int(rate * seconds)
    with wave.open(path, "wb") as w:
        w.setnchannels(channels)
        w.setsampwidth(2)
        w.setframerate(rate)
        pcm = bytearray()
        for i in range(n):
            level = int(20000 * math.sin(2.0 * math.pi * 220.0 * i / rate))
            pcm += struct.pack("<" + "h" * channels, *([level] * channels))
        w.writeframes(bytes(pcm))
    return path


def pump(data, stage=None):
    dst = io.BytesIO()
    frames = sd_filter.run(io.BytesIO(data), dst, log=None, stage=stage)
    return dst.getvalue(), frames


print("Plan 0106 - the diffusion filter")
print()
print("byte-identity through the pass-through, in-process:")

for w, h, n in [(16, 16, 3), (1920, 1080, 2), (2, 2, 1), (640, 360, 5)]:
    data = synth(w, h, n)
    out, frames = pump(data)
    check("%dx%d x%d round-trips byte-identically" % (w, h, n), out == data)
    check("%dx%d x%d frame count" % (w, h, n), frames == n, "%d != %d" % (frames, n))

# A zero-frame stream is a header and nothing else, and must survive.
hdr_only = b"YUV4MPEG2 W64 H64 F30:1 Ip A1:1 C444 XCOLORRANGE=FULL\n"
out, frames = pump(hdr_only)
check("header-only stream round-trips", out == hdr_only)
check("header-only stream is zero frames", frames == 0)

# A FRAME line carrying parameters this parser does not model must still be
# re-emitted verbatim - the reason header and marker are echoed, not rebuilt.
withparams = (
    b"YUV4MPEG2 W4 H4 F30:1 Ip A1:1 C444 XCOLORRANGE=FULL XSOMETAG=1\n"
    b"FRAME Xunknown=7\n" + bytes(range(48))
)
out, frames = pump(withparams)
check("unmodelled header/FRAME tags round-trip", out == withparams)

print()
print("geometry is read off the stream, not assumed:")

# The header, not a flag, decides the frame size: same geometry, different C
# tag, different bytes per frame. Asserted against the parser directly, because
# that is the property - deriving it from a total stream length only re-tests
# arithmetic this file would have to get right twice.
for colour, expect in [(b"444", 192), (b"422", 128), (b"420", 96), (b"mono", 64)]:
    hdr = b"YUV4MPEG2 W8 H8 F30:1 Ip A1:1 C%s" % colour + NL
    fmt = sd_filter.parse_header(hdr)
    check("8x8 C%s frames at %d bytes" % (colour.decode(), expect),
          (fmt.width, fmt.height, fmt.frame_bytes) == (8, 8, expect), "got %r" % (fmt,))
    check("8x8 C%s colour tag is carried" % colour.decode(), fmt.colour == colour)

# And the same tags survive a full pump, so the framing the parser computes is
# the framing the loop actually walks.
for colour in [b"444", b"422", b"420", b"mono"]:
    data = synth(8, 8, 2, colour=colour)
    out, n = pump(data)
    check("C%s round-trips through the loop" % colour.decode(), out == data and n == 2)

# A header with no C tag at all is C420 by the Y4M default, not an error.
fmt = sd_filter.parse_header(b"YUV4MPEG2 W8 H8 F30:1" + NL)
check("a header with no C tag defaults to 420",
      (fmt.width, fmt.height, fmt.frame_bytes) == (8, 8, 96), "got %r" % (fmt,))

print()
print("a malformed stream fails loudly:")

for name, blob in [
    ("not Y4M at all", b"NOTAY4M W4 H4\nFRAME\n"),
    ("header without geometry", b"YUV4MPEG2 F30:1 C444\nFRAME\n"),
    ("unknown colour space", b"YUV4MPEG2 W4 H4 C999\nFRAME\n"),
    ("truncated frame payload", b"YUV4MPEG2 W4 H4 C444\nFRAME\n" + b"\x00" * 10),
    ("garbage where FRAME belongs", b"YUV4MPEG2 W2 H2 C444\nNOPE\n" + b"\x00" * 12),
]:
    try:
        pump(blob)
        check(name, False, "did not raise")
    except sd_filter.StreamError:
        check(name, True)
    except Exception as e:  # noqa: BLE001
        check(name, False, "raised %r, not StreamError" % e)

print()
print("a pixel budget is spent at the stream's own aspect:")

# The rule ADR-0121 states, checked at the aspects Phase 2b and the ADR name.
# 1024x576 and 680x384 are the two shipping profiles at 16:9; 888x664 is the
# ADR's own 4:3 worked example. Each axis rounds to a multiple of 8 and the
# result lands within half a percent of the budget without a per-aspect table.
for (sw, sh), budget, expect in [
    ((1920, 1080), "589824", (1024, 576)),
    ((1280, 720), "589824", (1024, 576)),
    ((1920, 1080), "262144", (680, 384)),
    ((1600, 1200), "589824", (888, 664)),
    ((1080, 1920), "589824", (576, 1024)),
]:
    got = sd_filter.diffusion_size(sw, sh, budget)
    check("%dx%d at %s px -> %dx%d" % (sw, sh, budget, expect[0], expect[1]),
          got == expect, "got %r" % (got,))
    err = abs(got[0] * got[1] - int(budget)) / float(budget)
    check("  ... within 0.5%% of the budget", err <= 0.005, "off by %.3f%%" % (err * 100))
    check("  ... both axes are multiples of 8", got[0] % 8 == 0 and got[1] % 8 == 0)

# A WxH at the stream's aspect is accepted verbatim; one that disagrees is an
# error, because the whole finding of Phase 2b is that squashing costs both
# look and throughput. This is the flag that would otherwise squash silently.
check("an explicit WxH at the stream's aspect is taken as given",
      sd_filter.diffusion_size(1920, 1080, "1024x576") == (1024, 576))
for bad, why in [("768x768", "aspect disagrees"), ("1024x577", "not a multiple of 8"),
                 ("wide", "not a number"), ("1024x576x2", "not WxH")]:
    try:
        sd_filter.diffusion_size(1920, 1080, bad)
        check("--size %s is refused (%s)" % (bad, why), False, "did not raise")
    except sd_filter.ConfigError:
        check("--size %s is refused (%s)" % (bad, why), True)

print()
print("the colour conversion is pinned to its Rust twin:")

# THE TWIN OF THIS TABLE IS `the_colour_table_is_pinned_to_its_python_twin` in
# standalone/src/shot/render/tests.rs. It asserts these exact numbers against
# `rgb_to_yuv` / `yuv_to_rgb`. Neither file may be edited alone: that is the
# whole mechanism, and a one-sided edit reddens the side it was made on.
#
# Why it exists. This module's conversions are a second implementation of
# render.rs's, in another language, and until Plan 0106 Phase 7b nothing checked
# the pair. The pass-through never converts and the diffused path is
# unassertable, so a constant edited on one side ships as a colour cast across
# every frame and no instrument in this repo can see it.
#
# This is a PROPERTY, not a measurement (ADR-0071): the arithmetic is exact
# 8-bit integer output, identical on every machine, so it names no
# configuration and carries no tolerance.
#
# What this pin is sensitive to, measured rather than assumed: it reddens on any
# single-coefficient edit of +/-0.0005 or larger, in either direction, to any of
# the five forward constants. Below that it starts to miss - the worst case a
# +/-0.0002 luma edit can shift a channel is 0.05 of one 8-bit level, which is
# under the quantization floor of the format itself and cannot produce a cast
# the pin exists to catch. Structural errors - swapped Cb/Cr, BT.601 weights, a
# missing +128, a wrap where the clamp belongs - move these rows by tens of
# levels and are caught outright.
#
# What the rows are for: black, white and mid-grey (the neutral axis - a swapped
# coefficient moves them off it), the three primaries and three secondaries (the
# luma weights, and BT.709 against BT.601), and two saturated ramps. The clamp
# is the half most likely to be written differently in two languages, so it is
# exercised deliberately - see the note on each table below.

# RGB -> planar C444. Pure red's Cr computes to 255.5 and pure cyan's to 0.5, so
# those two rows are the whole forward-direction clamp: across the entire 8-bit
# cube the chroma terms reach exactly half a level past each end and no further.
RGB_TO_YUV = [
    ((0, 0, 0), (0, 128, 128)),
    ((255, 255, 255), (255, 128, 128)),
    ((128, 128, 128), (128, 128, 128)),
    ((255, 0, 0), (54, 99, 255)),       # Cr 255.5 -> clamped, not wrapped
    ((0, 255, 0), (182, 30, 12)),       # luma 182: BT.709; BT.601 would read 150
    ((0, 0, 255), (18, 255, 116)),
    ((0, 255, 255), (201, 157, 1)),     # Cr 0.5, the other end of the same edge
    ((255, 0, 255), (73, 226, 244)),
    ((255, 255, 0), (237, 1, 140)),
    ((250, 7, 7), (59, 100, 250)),
    ((7, 7, 250), (25, 250, 117)),
    ((18, 52, 86), (47, 149, 109)),
]

# Planar C444 -> RGB. This is where the clamp genuinely bites: an arbitrary YUV
# triple is not the image of any RGB one, so the terms leave 0..=255 by a wide
# margin. Five of these seven rows clamp at one end or the other, which is why
# the inverse direction carries its own table rather than being asserted as a
# round-trip of the one above.
YUV_TO_RGB = [
    ((0, 0, 0), (0, 84, 0)),            # R -201.6 and B -237.5, both clamped low
    ((255, 255, 255), (255, 172, 255)), # R 455 and B 490, both clamped high
    ((128, 128, 128), (128, 128, 128)),
    ((16, 240, 16), (0, 47, 224)),
    ((240, 16, 240), (255, 209, 32)),
    ((200, 20, 235), (255, 170, 0)),
    ((54, 128, 255), (254, 0, 54)),
]

try:
    import numpy as _np
except ImportError:
    print("  SKIPPED: no numpy, and the conversions under test are array functions")
    print("  (the Rust half of this pin still runs under `cargo test`)")
else:
    for rgb, yuv in RGB_TO_YUV:
        got = tuple(sd_filter.rgb_to_yuv444(_np.array([[rgb]], dtype=_np.uint8)))
        check("rgb %-15s -> yuv %s" % (rgb, yuv), got == yuv, "got %r" % (got,))
    for yuv, rgb in YUV_TO_RGB:
        arr = sd_filter.yuv444_to_rgb(bytes(bytearray(yuv)), 1, 1)
        got = tuple(int(c) for c in arr[0][0])
        check("yuv %-15s -> rgb %s" % (yuv, rgb), got == rgb, "got %r" % (got,))

print()
print("frames in equals frames out, at every stride:")


class CountingStage(sd_filter.DiffusionStage):
    """The real stride logic with the model taken out from under it.

    Only the three leaves that need a GPU or a decoder are replaced - reading a
    frame, diffusing it, encoding one back - so the accounting under test is the
    accounting that ships: `push`, `finish`, and `_crossfade`'s own count. The
    frame count is exactly the property that cannot be checked by eye, because a
    filter that dropped one would desynchronize the mux silently.
    """

    def begin(self, fmt, log=None):
        self.fmt = fmt
        self.dw, self.dh = fmt.width, fmt.height
        self.diffused = 0

    def _read(self, planes):
        return planes

    def _diffuse(self, src):
        self.diffused += 1
        # `push` has already advanced the index, so the frame being diffused is
        # the one before it. Naming the anchor after its own source frame is
        # what makes the expected patterns below readable.
        return b"D%d" % (self.index - 1)

    def _emit(self, img):
        return img.ljust(self.fmt.frame_bytes, b".")[: self.fmt.frame_bytes]

    def _blend(self, a, b, t):
        return b"%s+%s@%.2f" % (a, b, t)


def cell(**kw):
    cfg = dict(sd_filter.BASE)
    cfg.update(prompt="x", controlnet="n", **kw)
    return cfg


for gap in ["held", "blend"]:
    for stride in [1, 2, 3, 5, 8]:
        for n in [0, 1, 2, 7, 30, 31]:
            stage = CountingStage(cell(stride=stride, gap=gap))
            data = synth(8, 8, n)
            out, frames = pump(data, stage=stage)
            check("gap=%s stride=%d: %d in -> %d out" % (gap, stride, n, frames),
                  frames == n, "%d != %d" % (frames, n))
            # Diffusing every Nth frame is where the saving comes from; if this
            # drifts, the cost model in docs/capturing.md is wrong.
            want = (n + stride - 1) // stride
            check("  ... diffused %d of %d" % (stage.diffused, n),
                  stage.diffused == want, "%d != %d" % (stage.diffused, want))

# What the two gap fillers actually put in the gap, spelled out on one short
# stream: `held` repeats its anchor, `blend` walks from one anchor to the next.
# The counts above prove nothing about the content, and the content is the
# difference the user was asked to judge.
stage = CountingStage(cell(stride=3, gap="held"))
out, _ = pump(synth(8, 8, 7), stage=stage)
payloads = [f[:8] for f in out.split(b"FRAME\n")[1:]]
check("held repeats its anchor across the gap",
      payloads == [b"D0......", b"D0......", b"D0......",
                   b"D3......", b"D3......", b"D3......", b"D6......"],
      "got %r" % (payloads,))

stage = CountingStage(cell(stride=3, gap="blend"))
out, _ = pump(synth(8, 8, 7), stage=stage)
payloads = [f.rstrip(b".") for f in out.split(b"FRAME\n")[1:]]
check("blend crossfades between the anchors on either side",
      payloads == [b"D0", b"D0+D3@0.33", b"D0+D3@0.67",
                   b"D3", b"D3+D6@0.33", b"D3+D6@0.67", b"D6"],
      "got %r" % (payloads,))
check("blend's tail is held, because there is no next anchor",
      pump(synth(8, 8, 8), stage=CountingStage(cell(stride=3, gap="blend")))[1] == 8)

print()
print("a profile is reproducible from its own echo:")

parser = sd_filter.build_parser()

for name in sorted(sd_filter.PROFILES):
    args = parser.parse_args(["--profile", name, "--prompt", "a canyon, 'quoted'"])
    cfg = sd_filter.resolve(args)
    # The done-when: the echoed flags, passed back WITHOUT --profile, are the
    # same cell. A profile whose meaning moves later cannot invalidate a render
    # that recorded this line.
    echoed = sd_filter.expansion(cfg)
    again = sd_filter.resolve(parser.parse_args(shlex.split(echoed)))
    check("--profile %s round-trips through its expansion" % name, again == cfg,
          "%r" % ({k: (cfg[k], again[k]) for k in cfg if cfg[k] != again[k]},))
    check("  ... the echo names every flag the cell has",
          all(("--" + k.replace("_", "-")) in echoed
              for k in cfg if k not in ("lcm_lora",)),
          echoed)

# An explicit flag beats the profile it was passed alongside - the reason the
# profile is a preset and not a mode.
over = sd_filter.resolve(parser.parse_args(
    ["--profile", "fast", "--prompt", "p", "--stride", "1", "--steps", "12"]))
check("an explicit flag overrides the profile",
      (over["stride"], over["steps"], over["scheduler"]) == (1, 12, "lcm"),
      "%r" % (over,))

# The controlnet follows --control unless it is named, so the two cannot drift
# apart silently into a canny map driving a softedge net.
check("--control picks its own net",
      sd_filter.resolve(parser.parse_args(
          ["--prompt", "p", "--control", "canny"]))["controlnet"]
      == sd_filter.CONTROLNETS["canny"])

for bad, why in [
    (["--control", "canny"], "no prompt"),
    (["--prompt", "p", "--stride", "0"], "stride below 1"),
    (["--prompt", "p", "--feedback", "1.0"], "feedback of 1.0 never renders"),
    (["--prompt", "p", "--strength", "0"], "strength of 0 diffuses nothing"),
    (["--prompt", "p", "--steps", "0"], "no steps"),
]:
    try:
        sd_filter.resolve(parser.parse_args(bad))
        check("refused: %s" % why, False, "did not raise")
    except sd_filter.ConfigError:
        check("refused: %s" % why, True)

print()
print("end to end, as a subprocess (Phase 3's done-when as written):")

shot = os.path.join(REPO, "target", "release", "examples", "shot.exe")
if not os.path.exists(shot):
    shot = os.path.join(REPO, "target", "release", "examples", "shot")

if not os.path.exists(shot):
    print("  SKIPPED: no built `shot` at target/release/examples/")
    print("  build it with: cargo build -p standalone --release --example shot")
    print("  (the in-process checks above still ran and are the same property)")
else:
    with tempfile.TemporaryDirectory() as td:
        wav = synth_wav(os.path.join(td, "clip.wav"))
        args = [
            shot, "--preset-file",
            os.path.join(REPO, "presets", "attractor_leviathan.toml"),
            "--render", wav, "--fps", "30", "--size", "256x144", "--tier", "rich",
        ]
        direct = subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        filtered = subprocess.run(
            [sys.executable, os.path.join(HERE, "sd_filter.py"), "--passthrough"],
            input=direct.stdout, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        )
        check("shot exits 0", direct.returncode == 0)
        check("filter exits 0", filtered.returncode == 0)
        check("bytes through the filter are identical",
              filtered.stdout == direct.stdout,
              "%d vs %d bytes" % (len(filtered.stdout), len(direct.stdout)))
        check("the stream was not empty", len(direct.stdout) > 1000,
              "%d bytes" % len(direct.stdout))

    # Asking for a render without saying what to render is a configuration
    # error and exits 2, distinct from a malformed stream's 1 - and it must not
    # cost a multi-gigabyte weight download to find out.
    noprompt = subprocess.run(
        [sys.executable, os.path.join(HERE, "sd_filter.py")],
        input=b"", stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    check("no --prompt exits 2 before loading anything", noprompt.returncode == 2,
          "exit %d, stderr %r" % (noprompt.returncode, noprompt.stderr[-200:]))

print()
if FAILURES:
    print("FAILED (%d):" % len(FAILURES))
    for f in FAILURES:
        print("  - %s" % f)
    sys.exit(1)
print("all checks passed")
