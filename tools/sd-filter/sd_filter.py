#!/usr/bin/env python3
"""Plan 0106 Phase 3 - the pass-through stub.

Reads Plan 0101's Y4M frame stream on stdin, writes it unchanged to stdout.
No model, no weights, no GPU: this stage exists to prove the plumbing, and it
is the only part of this feature whose output is reproducible across machines
and therefore gateable.

The stub deliberately *parses* rather than copying. `shutil.copyfileobj` would
satisfy byte-identity while proving nothing, and Phase 4 replaces the middle of
this loop with a diffusion call - so the frame boundaries have to be real here
or they are simply unwritten work wearing a passing test.

Header and FRAME lines are re-emitted as the exact bytes they arrived as, not
reserialized from the parsed fields. A stream carrying a tag this parser does
not model still round-trips byte-for-byte.
"""

import sys

MAGIC = b"YUV4MPEG2"

# Bytes per pixel, as a fraction, for each colour-space tag ffmpeg's Y4M muxer
# will emit. `shot` writes C444 (ADR-0114: chroma is not subsampled, so the
# RGB->YUV conversion loses only rounding); the rest are here so a stream from
# some other producer fails on its own terms rather than on a silent misread.
PLANE_RATIO = {
    b"420": 3 / 2,
    b"420jpeg": 3 / 2,
    b"420mpeg2": 3 / 2,
    b"420paldv": 3 / 2,
    b"411": 3 / 2,
    b"422": 2,
    b"444": 3,
    b"mono": 1,
}


class StreamError(Exception):
    """The stream is not what its own header says it is."""


def read_line(src):
    """Read one `\n`-terminated line, returning it with the terminator.

    Y4M headers are text lines inside a binary stream, so this reads a byte at
    a time rather than buffering ahead - over-reading here would eat the first
    bytes of a frame's payload.
    """
    line = bytearray()
    while True:
        b = src.read(1)
        if not b:
            if not line:
                return None  # clean EOF, on a boundary
            raise StreamError("stream ended mid-line: %r" % bytes(line))
        line += b
        if b == b"\n":
            return bytes(line)


def parse_header(line):
    """Return (width, height, frame_bytes) from a YUV4MPEG2 header line."""
    fields = line.rstrip(b"\n").split(b" ")
    if not fields or fields[0] != MAGIC:
        raise StreamError("not a Y4M stream: header begins %r" % line[:32])

    width = height = None
    colour = b"420"  # the Y4M default when no C tag is present
    for f in fields[1:]:
        if f[:1] == b"W":
            width = int(f[1:])
        elif f[:1] == b"H":
            height = int(f[1:])
        elif f[:1] == b"C":
            colour = f[1:]

    if width is None or height is None:
        raise StreamError("header names no geometry: %r" % line)
    if colour not in PLANE_RATIO:
        raise StreamError(
            "unsupported colour space C%s (known: %s)"
            % (colour.decode("ascii", "replace"),
               ", ".join(sorted(k.decode() for k in PLANE_RATIO)))
        )

    frame_bytes = int(width * height * PLANE_RATIO[colour])
    return width, height, frame_bytes


def read_exactly(src, n):
    """Read exactly n bytes or raise - a short read here is a truncated frame."""
    buf = bytearray()
    while len(buf) < n:
        chunk = src.read(n - len(buf))
        if not chunk:
            raise StreamError(
                "stream ended mid-frame: wanted %d bytes, got %d" % (n, len(buf))
            )
        buf += chunk
    return bytes(buf)


def transform(planes, _width, _height):
    """The seam Phase 4 replaces. Identity, and that is the entire point."""
    return planes


def run(src, dst, log=None):
    """Pump one Y4M stream from src to dst. Returns the frame count."""
    header = read_line(src)
    if header is None:
        raise StreamError("empty stream: no Y4M header")
    width, height, frame_bytes = parse_header(header)
    dst.write(header)

    if log:
        print(
            "sd-filter: %dx%d, %d bytes/frame" % (width, height, frame_bytes),
            file=log,
            flush=True,
        )

    frames = 0
    while True:
        marker = read_line(src)
        if marker is None:
            break  # clean EOF on a frame boundary
        if not marker.startswith(b"FRAME"):
            raise StreamError(
                "expected FRAME at frame %d, found %r" % (frames, marker[:32])
            )
        planes = read_exactly(src, frame_bytes)
        dst.write(marker)
        dst.write(transform(planes, width, height))
        frames += 1

    dst.flush()
    if log:
        print("sd-filter: %d frames" % frames, file=log, flush=True)
    return frames


def main(argv):
    if len(argv) > 1 and argv[1] in ("-h", "--help"):
        print(__doc__, file=sys.stderr)
        return 0

    # On Windows a stdio handle inherited in text mode turns every 0x0A in a
    # frame's payload into 0x0D 0x0A, which corrupts the picture and passes
    # silently. `.buffer` avoids Python's own translation layer; this pins the
    # handle itself.
    if sys.platform == "win32":
        import msvcrt
        import os

        for fh in (sys.stdin, sys.stdout):
            msvcrt.setmode(fh.fileno(), os.O_BINARY)

    try:
        run(sys.stdin.buffer, sys.stdout.buffer, log=sys.stderr)
    except StreamError as e:
        print("sd-filter: %s" % e, file=sys.stderr)
        return 1
    except BrokenPipeError:
        # The sink closed early (ffmpeg hit an error, or a head(1)-alike). Not
        # this stage's failure to report.
        return 0
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
