#!/usr/bin/env python3
"""Plan 0106 Phase 3 done-when, as an executable check.

Runs with the standard library only and needs no GPU, no weights and no model -
which is the property that makes this stage, alone in this feature, gateable.

    python tools/sd-filter/test_sd_filter.py

The end-to-end half needs a built `shot`; without one it SKIPS with a printed
notice rather than passing quietly (ADR-0016), because a check that reports
success when it did not run is worse than one that is absent.
"""

import io
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)

import sd_filter  # noqa: E402

FAILURES = []
NL = bytes([10])  # written this way so the escape survives the shell heredoc


def check(name, cond, detail=""):
    print("  %-46s %s" % (name, "ok" if cond else "FAIL"))
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


def pump(data):
    dst = io.BytesIO()
    frames = sd_filter.run(io.BytesIO(data), dst, log=None)
    return dst.getvalue(), frames


print("Plan 0106 Phase 3 - the pass-through stub")
print()
print("byte-identity, in-process:")

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
    w, h, fb = sd_filter.parse_header(hdr)
    check("8x8 C%s frames at %d bytes" % (colour.decode(), expect),
          (w, h, fb) == (8, 8, expect), "got %r" % ((w, h, fb),))

# And the same tags survive a full pump, so the framing the parser computes is
# the framing the loop actually walks.
for colour in [b"444", b"422", b"420", b"mono"]:
    data = synth(8, 8, 2, colour=colour)
    out, n = pump(data)
    check("C%s round-trips through the loop" % colour.decode(), out == data and n == 2)

# A header with no C tag at all is C420 by the Y4M default, not an error.
w, h, fb = sd_filter.parse_header(b"YUV4MPEG2 W8 H8 F30:1" + NL)
check("a header with no C tag defaults to 420", (w, h, fb) == (8, 8, 96),
      "got %r" % ((w, h, fb),))

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
print("end to end, as a subprocess (the done-when as written):")

shot = os.path.join(REPO, "target", "release", "examples", "shot.exe")
if not os.path.exists(shot):
    shot = os.path.join(REPO, "target", "release", "examples", "shot")

if not os.path.exists(shot):
    print("  SKIPPED: no built `shot` at target/release/examples/")
    print("  build it with: cargo build -p standalone --release --example shot")
    print("  (the in-process checks above still ran and are the same property)")
else:
    with tempfile.TemporaryDirectory() as td:
        wav = os.path.join(REPO, "spike", "clip.wav")
        args = [
            shot, "--preset-file",
            os.path.join(REPO, "presets", "attractor_leviathan.toml"),
            "--render", wav, "--fps", "30", "--size", "256x144", "--tier", "rich",
        ]
        direct = subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        filtered = subprocess.run(
            [sys.executable, os.path.join(HERE, "sd_filter.py")],
            input=direct.stdout, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        )
        check("shot exits 0", direct.returncode == 0)
        check("filter exits 0", filtered.returncode == 0)
        check("bytes through the filter are identical",
              filtered.stdout == direct.stdout,
              "%d vs %d bytes" % (len(filtered.stdout), len(direct.stdout)))
        check("the stream was not empty", len(direct.stdout) > 1000,
              "%d bytes" % len(direct.stdout))

print()
if FAILURES:
    print("FAILED (%d):" % len(FAILURES))
    for f in FAILURES:
        print("  - %s" % f)
    sys.exit(1)
print("all checks passed")
