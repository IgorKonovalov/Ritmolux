#!/usr/bin/env python3
"""Plan 0106 - the diffusion filter.

Reads Plan 0101's Y4M frame stream on stdin, writes the same stream format, the
same geometry and the same frame count on stdout. Between the two it runs an
img2img pass with ControlNet holding the render's geometry, so an attractor or a
mandala becomes material while the shape keeps tracking the music.

    shot --render track.wav ... | python sd_filter.py --profile quality
        --prompt "a vast canyon of luminous glowing rock strata" | ffmpeg ...

`--passthrough` keeps Phase 3's stub behaviour: parse the stream, emit it
unchanged, no model and no GPU. That path is byte-exact and reproducible across
machines, which makes it the only part of this feature that can be a real gate
(ADR-0121); everything the model touches is not, so nothing here may ever become
a golden baseline.

Three contracts this file exists to keep, all from ADR-0121:

  * **Frames in equals frames out, always.** `--stride N` diffuses every Nth
    frame and fills the gap itself. A filter that changed the rate would
    desynchronize the audio mux downstream, and that failure is silent in the
    file - nothing reports it and no test in this repo would see it.
  * **Resolution is a pixel budget spent at the stream's own aspect.** A profile
    names a pixel count, never a side length. The diffusion geometry is derived
    from the budget and the incoming header, each axis rounded to a multiple of
    8, and the frame is never squashed or letterboxed: Phase 2b measured the
    square arms slower AND less detailed at an identical pixel count.
  * **A render is reproducible from its own stderr.** The expanded flag list is
    echoed before the first frame, so a file's configuration is recoverable
    without knowing which version of a profile was current when it ran.

Progress goes to stderr. stdout carries frames and nothing else.
"""

import argparse
import math
import shlex
import sys
from collections import deque, namedtuple

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

StreamFormat = namedtuple("StreamFormat", "width height frame_bytes colour")

CONTROLNETS = {
    "canny": "lllyasviel/control_v11p_sd15_canny",
    "softedge": "lllyasviel/control_v11p_sd15_softedge",
    "lineart": "lllyasviel/control_v11p_sd15_lineart",
}

DEFAULT_NEGATIVE = "text, watermark, blurry, low quality, frame, border"

# A profile is a set of values for the flags below and nothing else - never a
# second surface with its own behaviour. Both are the LCM cfg 2.0 cell Plan
# 0106's look gate approved, measured on the dev box rather than guessed; they
# differ in pixel budget, feedback and stride, which is to say in what they
# spend and how much they persist, not in what they draw.
PROFILES = {
    # 589,824 px is 1024x576 at 16:9. Feedback 0.6 measures boil 0.93 - the
    # output persists rather than chases, which is what the gate asked for after
    # it complained about rate rather than about incoherence.
    #
    # The cell is LCM at cfg 2.0 and NOT the 20-step UniPC cell Plan 0106 Phase 4
    # named, because the two were rendered against each other at this geometry
    # and the 20-step cell does not survive the move off 512/768 square: it draws
    # contour line-work over the attractor rather than material, at feedback 0.4
    # and 0.6 alike, for 6.24 s/frame against 2.98. Retuned 2026-08-24 on the
    # user's call, with the comparison in the plan's implementation log. The
    # 20-step cell is still one flag away: --scheduler unipc --steps 20 --cfg 7.0.
    "quality": {
        "size": "589824",
        "scheduler": "lcm",
        "steps": 8,
        "cfg": 2.0,
        "strength": 0.75,
        "cn_scale": 0.6,
        "feedback": 0.6,
        "stride": 1,
        "control": "softedge",
        "model": "Lykon/dreamshaper-8",
    },
    # 262,144 px because that is the pixel count of the 512x512 cell the LCM
    # ladder was measured at; preserving the count preserves the measured cost,
    # which a rounder-looking 512x288 would not. cfg 2.0 is load-bearing: at
    # cfg 1.0 the same 8 steps return a flat, materialless picture, and a
    # dedicated guidance-free finetune fails the same way.
    "fast": {
        "size": "262144",
        "scheduler": "lcm",
        "steps": 8,
        "cfg": 2.0,
        "strength": 0.75,
        "cn_scale": 0.6,
        "feedback": 0.4,
        "stride": 3,
        "control": "softedge",
        "model": "Lykon/dreamshaper-8",
    },
}


class StreamError(Exception):
    """The stream is not what its own header says it is."""


class ConfigError(Exception):
    """The arguments do not describe a render this filter can perform."""


# ------------------------------------------------------------------ the wire


def read_line(src):
    """Read one newline-terminated line, returning it with the terminator.

    Y4M headers are text lines inside a binary stream, so this reads a byte at a
    time rather than buffering ahead - over-reading here would eat the first
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
    """Return the StreamFormat a YUV4MPEG2 header line describes."""
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
    return StreamFormat(width, height, frame_bytes, colour)


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


def run(src, dst, log=None, stage=None):
    """Pump one Y4M stream from src to dst through `stage`. Returns frames out.

    With no stage this is the pass-through: the header and every FRAME line are
    re-emitted as the exact bytes they arrived as rather than reserialized from
    the parsed fields, so a stream carrying a tag this parser does not model
    still round-trips byte-for-byte.

    A stage may hold frames back - a stride gap cannot be interpolated until the
    anchor after it exists - so markers wait in a queue and are spent in arrival
    order as payloads come out. The queue draining exactly at EOF is the
    frame-count contract, and it is asserted here rather than assumed.
    """
    header = read_line(src)
    if header is None:
        raise StreamError("empty stream: no Y4M header")
    fmt = parse_header(header)
    dst.write(header)

    if log:
        print(
            "sd-filter: stream %dx%d C%s, %d bytes/frame"
            % (fmt.width, fmt.height, fmt.colour.decode(), fmt.frame_bytes),
            file=log, flush=True,
        )
    if stage is not None:
        stage.begin(fmt, log)

    markers = deque()
    frames_in = frames_out = 0
    while True:
        marker = read_line(src)
        if marker is None:
            break  # clean EOF on a frame boundary
        if not marker.startswith(b"FRAME"):
            raise StreamError(
                "expected FRAME at frame %d, found %r" % (frames_in, marker[:32])
            )
        planes = read_exactly(src, fmt.frame_bytes)
        frames_in += 1
        markers.append(marker)

        for out in ([planes] if stage is None else stage.push(planes)):
            dst.write(markers.popleft())
            dst.write(out)
            frames_out += 1
            if log and stage is not None and frames_out % 10 == 0:
                print("sd-filter: %d frames" % frames_out, file=log, flush=True)

    if stage is not None:
        for out in stage.finish():
            dst.write(markers.popleft())
            dst.write(out)
            frames_out += 1
        stage.report(frames_out)

    dst.flush()
    if frames_out != frames_in or markers:
        # Not reachable by configuration - it would mean a stage lost or
        # invented a frame, which desynchronizes the mux silently downstream.
        raise StreamError(
            "frame count contract broken: %d in, %d out" % (frames_in, frames_out)
        )
    if log:
        print("sd-filter: done, %d frames" % frames_out, file=log, flush=True)
    return frames_out


# -------------------------------------------------------------- the geometry


def round8(v):
    """Nearest multiple of 8, at least 8 - the VAE's latent stride."""
    return max(8, int(round(v / 8.0)) * 8)


def diffusion_size(width, height, size):
    """Resolve `--size` against the stream's own aspect.

    A budget is a pixel count and the aspect comes from the header, which is the
    whole finding of Phase 2b: at an identical budget the native arm was both
    the cheapest (2.721 against 2.871 and 2.913 s/frame) and the only one
    delivering every pixel it paid for. A WxH that disagrees with the stream is
    an error rather than a silent squash, for the same reason.
    """
    if "x" in size.lower():
        parts = size.lower().split("x")
        if len(parts) != 2 or not all(p.isdigit() for p in parts):
            raise ConfigError("--size %r is neither a pixel budget nor WxH" % size)
        w, h = int(parts[0]), int(parts[1])
        if w % 8 or h % 8:
            raise ConfigError(
                "--size %dx%d is not a multiple of 8 on both axes (nearest: %dx%d)"
                % (w, h, round8(w), round8(h))
            )
        want, got = width / height, w / h
        if abs(want - got) / want > 0.01:
            raise ConfigError(
                "--size %dx%d is aspect %.3f but the stream is %dx%d, aspect %.3f. "
                "Squashing costs both look and throughput (Phase 2b); pass a pixel "
                "budget instead and the geometry follows the stream."
                % (w, h, got, width, height, want)
            )
        return w, h

    if not size.isdigit():
        raise ConfigError("--size %r is neither a pixel budget nor WxH" % size)
    budget = int(size)
    if budget < 4096:
        raise ConfigError("--size budget %d is too small to diffuse" % budget)
    scale = math.sqrt(budget / float(width * height))
    return round8(width * scale), round8(height * scale)


# --------------------------------------------------------------- the colours
#
# The stream is C444, full-range BT.709 (ADR-0114). These invert
# `standalone/src/shot/render.rs`'s `rgb_to_yuv` / `yuv_to_rgb` term for term,
# including its round-half-away-from-zero and its clamp: the chroma terms of a
# saturated pixel legitimately land outside 0..=255, and wrapping there is a
# wrong colour rather than a clipped one.


def _round_u8(a):
    import numpy as np

    return np.clip(np.floor(a + 0.5), 0, 255).astype(np.uint8)


def yuv444_to_rgb(planes, width, height):
    """Planar C444 bytes -> an (h, w, 3) uint8 RGB array."""
    import numpy as np

    a = np.frombuffer(planes, dtype=np.uint8).reshape(3, height, width)
    a = a.astype(np.float32)
    y = a[0]
    u = a[1] - 128.0
    v = a[2] - 128.0
    return _round_u8(np.stack([
        y + 1.5748 * v,
        y - 0.187324 * u - 0.468124 * v,
        y + 1.8556 * u,
    ], axis=-1))


def rgb_to_yuv444(rgb):
    """An (h, w, 3) uint8 RGB array -> planar C444 bytes."""
    import numpy as np

    a = rgb.astype(np.float32)
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    y = 0.2126 * r + 0.7152 * g + 0.0722 * b
    return _round_u8(np.stack([
        y,
        (b - y) / 1.8556 + 128.0,
        (r - y) / 1.5748 + 128.0,
    ], axis=0)).tobytes()


# ----------------------------------------------------------------- the stage


class DiffusionStage:
    """The img2img pass, as a stateful stream filter.

    The model loads once, before the first frame, and the previous output is
    held across frames: this is not a request/response service, and reloading
    per frame would dominate the runtime completely.

    The coherence recipe is the spike's, unchanged. The CONTROL image comes from
    *this* frame's render, so the geometry tracks the music; the img2img BASE
    carries the *previous* output blended in, so material persists. One fixed
    seed for the whole render - the frame-to-frame difference is then the
    render's, not the sampler's.
    """

    def __init__(self, cfg):
        self.cfg = cfg
        self.pipe = None
        self.detector = None
        self.fmt = None
        self.dw = self.dh = 0
        self.resample = None
        self.prev = None     # previous diffused output, in diffusion space
        self.held = None     # last emitted frame, as plane bytes (gap: held)
        self.anchor = None   # last diffused output, as an image (gap: blend)
        self.gaps = 0        # gap frames waiting on the next anchor
        self.index = 0
        self.times = []      # seconds inside the diffusion call, per diffused frame
        self.started = None  # perf_counter at the head of the stream (wall clock)
        self.load_seconds = 0.0   # of that wall clock, what the model build took
        self.log = None

    # -- lifecycle

    def begin(self, fmt, log=None):
        import time

        from PIL import Image

        # The wall clock starts here, at the head of the stream, and it is the
        # only timer that sees everything this stage does (Plan 0106 Phase 7d).
        self.started = time.perf_counter()
        if fmt.colour != b"444":
            raise ConfigError(
                "diffusion needs a C444 stream (this one is C%s); `shot --render` "
                "writes C444" % fmt.colour.decode()
            )
        self.fmt = fmt
        self.log = log
        self.resample = Image.LANCZOS
        self.dw, self.dh = diffusion_size(fmt.width, fmt.height, self.cfg["size"])
        if log:
            same = (self.dw, self.dh) == (fmt.width, fmt.height)
            print(
                "sd-filter: diffusing at %dx%d (%d px)%s"
                % (self.dw, self.dh, self.dw * self.dh,
                   "" if same else
                   ", resampled from %dx%d and back" % (fmt.width, fmt.height)),
                file=log, flush=True,
            )
        build_started = time.perf_counter()
        self._build(log)
        self.load_seconds = time.perf_counter() - build_started

    def _build(self, log):
        import torch
        from diffusers import (
            ControlNetModel,
            LCMScheduler,
            StableDiffusionControlNetImg2ImgPipeline,
            UniPCMultistepScheduler,
        )

        cfg = self.cfg

        if not torch.cuda.is_available():
            raise ConfigError(
                "no CUDA device: torch %s reports cuda unavailable. A CPU-only "
                "install of torch is the usual cause - see requirements.txt, which "
                "pins the +cu124 build and its index URL for exactly this reason."
                % torch.__version__
            )

        def fp16_first(loader, repo, **kw):
            # The fp16 variant halves the download; not every repo publishes one
            # (`Lykon/dreamshaper-8` does not).
            try:
                return loader.from_pretrained(
                    repo, variant="fp16", torch_dtype=torch.float16, **kw
                )
            except Exception:
                if log:
                    print("sd-filter: no fp16 variant for %s, full weights" % repo,
                          file=log, flush=True)
                return loader.from_pretrained(repo, torch_dtype=torch.float16, **kw)

        net = fp16_first(ControlNetModel, cfg["controlnet"])
        pipe = fp16_first(
            StableDiffusionControlNetImg2ImgPipeline,
            cfg["model"],
            controlnet=net,
            safety_checker=None,
            requires_safety_checker=False,
        )
        if cfg["scheduler"] == "lcm":
            # Latent consistency: a distilled schedule. It does NOT remove the
            # guidance cost - the spike measured cfg 1.0 flat at every strength
            # tried, on the LoRA and on a dedicated finetune alike - so the
            # profile that uses it still runs cfg 2.0.
            if cfg["lcm_lora"]:
                pipe.load_lora_weights(cfg["lcm_lora"])
                pipe.fuse_lora()
            pipe.scheduler = LCMScheduler.from_config(pipe.scheduler.config)
        else:
            pipe.scheduler = UniPCMultistepScheduler.from_config(pipe.scheduler.config)
        pipe.set_progress_bar_config(disable=True)
        # NOT enable_model_cpu_offload: correct output, ruinous throughput once
        # it is paid per frame across thousands of frames.
        pipe.to("cuda")
        net.to("cuda")
        self.pipe = pipe

    # -- the control map

    def _control(self, img):
        from PIL import Image

        kind = self.cfg["control"]
        if kind == "canny":
            import cv2
            import numpy as np

            edges = cv2.Canny(np.array(img.convert("RGB")), 80, 160)
            ctrl = Image.fromarray(np.stack([edges] * 3, axis=-1))
        else:
            if self.detector is None:
                from controlnet_aux import HEDdetector, LineartDetector

                cls = HEDdetector if kind == "softedge" else LineartDetector
                self.detector = cls.from_pretrained("lllyasviel/Annotators").to("cuda")
            long_side = max(img.size)
            ctrl = self.detector(
                img, detect_resolution=long_side, image_resolution=long_side
            )
        if ctrl.size != img.size:
            ctrl = ctrl.resize(img.size, self.resample)
        return ctrl

    # -- one diffused frame

    def _diffuse(self, src):
        import time

        import torch
        from PIL import Image

        cfg = self.cfg
        base = src if self.prev is None else Image.blend(src, self.prev, cfg["feedback"])
        started = time.perf_counter()
        out = self.pipe(
            prompt=cfg["prompt"],
            negative_prompt=cfg["negative"],
            image=base,
            control_image=self._control(src),
            strength=cfg["strength"],
            controlnet_conditioning_scale=cfg["cn_scale"],
            num_inference_steps=cfg["steps"],
            guidance_scale=cfg["cfg"],
            generator=torch.Generator("cuda").manual_seed(cfg["seed"]),
        ).images[0]
        self.times.append(time.perf_counter() - started)
        self.prev = out
        return out

    def report(self, emitted=0):
        """What this run cost, as the wall clock sees it and as the model does.

        TWO numbers, deliberately not collapsed into one (Plan 0106 Phase 7d).
        They measure different things and the difference is the finding:

        - the WALL CLOCK covers the whole stream, so it carries `_read`'s colour
          decode and downscale, `_emit`'s upscale back to the stream's geometry
          and its RGB->YUV encode, the gap crossfades, and the model load. `_emit`
          runs per EMITTED frame, so at stride N one anchor pays N full-resolution
          upscales and N full-frame colour encodes;
        - the DIFFUSION MEAN times `self.pipe(...)` alone, and nothing else.

        Until this phase only the second existed, and it was divided by the stride
        and printed as "per emitted frame" - which is the label the documents took
        and it was wrong about its own scope. On the Phase 6 render the two
        differed by 1.406x, and one number cannot carry both.

        Both are measurements naming their configuration, never properties
        (ADR-0071): seconds on one machine, one model, one geometry.
        """
        if not (self.log and self.times):
            return
        import time

        mean = sum(self.times) / len(self.times)
        diffused = len(self.times)

        if self.started is not None and emitted > 0:
            elapsed = time.perf_counter() - self.started
            print(
                "sd-filter: %d emitted in %.1f s = %.3f s per emitted frame, WALL "
                "CLOCK (model load %.1f s of that)"
                % (emitted, elapsed, elapsed / emitted, self.load_seconds),
                file=self.log, flush=True,
            )

        note = ""
        if self.pipe is not None:
            import torch

            note = ", peak VRAM %.2f GiB" % (torch.cuda.max_memory_reserved() / 2 ** 30)
        print(
            "sd-filter: %d diffused, mean %.3f s in the diffusion CALL alone "
            "(stride %d)%s"
            % (diffused, mean, self.cfg["stride"], note),
            file=self.log, flush=True,
        )
        print(
            "sd-filter: the gap between the two is colour conversion and "
            "resampling, which the wall clock counts and the call timer does not",
            file=self.log, flush=True,
        )

    # -- the stream shape

    def _read(self, planes):
        """The stream's geometry -> what the model sees."""
        from PIL import Image

        img = Image.fromarray(
            yuv444_to_rgb(planes, self.fmt.width, self.fmt.height), "RGB"
        )
        if (self.dw, self.dh) != img.size:
            img = img.resize((self.dw, self.dh), self.resample)
        return img

    def _emit(self, img):
        """Diffusion space -> the stream's geometry, as C444 plane bytes."""
        import numpy as np

        if img.size != (self.fmt.width, self.fmt.height):
            img = img.resize((self.fmt.width, self.fmt.height), self.resample)
        return rgb_to_yuv444(np.asarray(img.convert("RGB")))

    def push(self, planes):
        """Consume one input frame; return the output frames it releases.

        held  : one out per one in, no delay. A gap repeats its anchor, which is
                a deliberately stepped 30/N look and costs no dependency.
        blend : a gap crossfades between the anchors on either side of it, so
                nothing can be emitted until the *next* anchor exists. That is a
                delay of N frames, never a change in their number.
        """
        i = self.index
        self.index += 1
        # A gap frame's own pixels are never looked at: the frame it stands in
        # for has already been diffused, or is about to be. Reading it would
        # cost a colour conversion and a resample per frame for nothing.
        is_anchor = i % self.cfg["stride"] == 0

        if self.cfg["gap"] == "held":
            if is_anchor:
                self.held = self._emit(self._diffuse(self._read(planes)))
            return [self.held]

        if not is_anchor:
            self.gaps += 1
            return []

        out = self._diffuse(self._read(planes))
        released = [] if self.anchor is None else self._crossfade(self.anchor, out)
        self.anchor = out
        self.gaps = 0
        return released

    def _blend(self, a, b, t):
        from PIL import Image

        return Image.blend(a, b, t)

    def _crossfade(self, a, b):
        """The frames from anchor `a` up to, but not including, anchor `b`."""
        n = self.gaps + 1
        frames = [self._emit(a)]
        for k in range(1, n):
            frames.append(self._emit(self._blend(a, b, k / float(n))))
        return frames

    def finish(self):
        """The tail after the last anchor: held, because there is no next one."""
        if self.cfg["gap"] == "held" or self.anchor is None:
            return []
        return [self._emit(self.anchor)] * (self.gaps + 1)


# ----------------------------------------------------------------- the flags

# The order the echo prints in, and the set a profile is made of. `--cfg` and
# `--scheduler` are here and not in the plan's illustrative list because the two
# profiles differ in exactly those: an echo that omitted them would name a cell
# it could not reproduce.
FLAG_ORDER = [
    ("model", "--model"),
    ("controlnet", "--controlnet"),
    ("control", "--control"),
    ("scheduler", "--scheduler"),
    ("steps", "--steps"),
    ("cfg", "--cfg"),
    ("strength", "--strength"),
    ("cn_scale", "--cn-scale"),
    ("feedback", "--feedback"),
    ("stride", "--stride"),
    ("gap", "--gap"),
    ("size", "--size"),
    ("seed", "--seed"),
    ("prompt", "--prompt"),
    ("negative", "--negative"),
]

BASE = {
    "prompt": None,
    "negative": DEFAULT_NEGATIVE,
    "model": "Lykon/dreamshaper-8",
    "controlnet": None,
    "control": "softedge",
    "scheduler": "unipc",
    "steps": 20,
    "cfg": 7.0,
    "strength": 0.75,
    "cn_scale": 0.6,
    "feedback": 0.4,
    "stride": 1,
    "gap": "blend",
    "size": "589824",
    "seed": 1234,
    "lcm_lora": "latent-consistency/lcm-lora-sdv1-5",
}


def build_parser():
    p = argparse.ArgumentParser(
        prog="sd_filter.py",
        description="Plan 0106's diffusion filter: a Y4M stage that sits between "
                    "`shot --render` and `ffmpeg`.",
    )
    p.add_argument("--passthrough", action="store_true",
                   help="emit the stream unchanged; no model, no GPU (Phase 3)")
    p.add_argument("--profile", choices=sorted(PROFILES),
                   help="a known-good combination of the flags below; any flag "
                        "passed explicitly overrides it")
    p.add_argument("--prompt")
    p.add_argument("--negative")
    p.add_argument("--model")
    p.add_argument("--controlnet", help="defaults to the net for --control")
    p.add_argument("--control", choices=sorted(CONTROLNETS))
    p.add_argument("--scheduler", choices=["unipc", "lcm"])
    p.add_argument("--steps", type=int)
    p.add_argument("--cfg", type=float, help="classifier-free guidance scale")
    p.add_argument("--strength", type=float)
    p.add_argument("--cn-scale", dest="cn_scale", type=float)
    p.add_argument("--feedback", type=float)
    p.add_argument("--stride", type=int,
                   help="diffuse every Nth frame; N are still emitted")
    p.add_argument("--gap", choices=["held", "blend"],
                   help="how a stride gap is filled (default blend)")
    p.add_argument("--size",
                   help="a pixel budget (589824) or WxH at the stream's aspect")
    p.add_argument("--seed", type=int)
    p.add_argument("--lcm-lora", dest="lcm_lora",
                   help="the LoRA fused when --scheduler lcm and --model is a base")
    return p


def resolve(args):
    """Base, then profile, then explicit flags. Returns the cell."""
    cfg = dict(BASE)
    if args.profile:
        cfg.update(PROFILES[args.profile])
    for key in list(cfg):
        got = getattr(args, key, None)
        if got is not None:
            cfg[key] = got

    if not cfg["prompt"]:
        raise ConfigError(
            "--prompt is required (the image is the whole signal, so there is no "
            "default worth having). Pass --passthrough for the no-model stage."
        )
    if cfg["control"] not in CONTROLNETS:
        raise ConfigError("--control %r is not one of %s"
                          % (cfg["control"], ", ".join(sorted(CONTROLNETS))))
    if cfg["controlnet"] is None:
        cfg["controlnet"] = CONTROLNETS[cfg["control"]]
    if cfg["stride"] < 1:
        raise ConfigError("--stride must be at least 1, got %d" % cfg["stride"])
    if not 0.0 <= cfg["feedback"] < 1.0:
        raise ConfigError("--feedback must be in [0, 1), got %r" % cfg["feedback"])
    if not 0.0 < cfg["strength"] <= 1.0:
        raise ConfigError("--strength must be in (0, 1], got %r" % cfg["strength"])
    if cfg["steps"] < 1:
        raise ConfigError("--steps must be at least 1, got %d" % cfg["steps"])
    return cfg


def expansion(cfg):
    """The cell as the flag list that reproduces it, for the stderr echo.

    Quoted so `shlex.split` returns the argv that produced this render. A
    profile name whose meaning has since moved is not a configuration; this is
    the difference between the two, and the reason the echo is not optional.
    """
    out = []
    for key, flag in FLAG_ORDER:
        out += [flag, shlex.quote(str(cfg[key]))]
    if cfg["scheduler"] == "lcm":
        out += ["--lcm-lora", shlex.quote(str(cfg["lcm_lora"]))]
    return " ".join(out)


def main(argv):
    args = build_parser().parse_args(argv[1:])

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
        stage = None
        if not args.passthrough:
            cfg = resolve(args)
            print("sd-filter: %s" % expansion(cfg), file=sys.stderr, flush=True)
            stage = DiffusionStage(cfg)
        run(sys.stdin.buffer, sys.stdout.buffer, log=sys.stderr, stage=stage)
    except ConfigError as e:
        print("sd-filter: %s" % e, file=sys.stderr)
        return 2
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
