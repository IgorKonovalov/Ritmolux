# `sd-filter` — the frame stream passes through a diffusion model

A stage that sits between `shot --render` and `ffmpeg`. Attractors and mandalas go
in; an img2img pass with ControlNet holding their geometry turns them into
material — a canyon, a cathedral rose window, a creature — while the shape keeps
tracking the music.

**Everything about it is documented in one place:
[`docs/diffusion-filter.md`](../../docs/diffusion-filter.md).** The canonical
command, the profiles, the flags, what it costs and what the check covers all live
there and only there, so a correction has one place to land
([ADR-0122](../../docs/adrs/0122-a-sidecar-tool-documents-itself-in-one-place.md)).
This file is how to install it, and nothing else.

**Nothing here ships.** No model, no weights and no Python runtime are in the
release zip; `lmv.exe` and `foo_lmv.dll` do not change size, and
[NFR §4](../../docs/nfr.md#4-size-and-dependencies)'s size budget is untouched.
This is creator tooling you build yourself.

## Install

```bash
python -m venv .venv
.venv/Scripts/python -m pip install -r tools/sd-filter/requirements.txt   # Windows
```

It needs a CUDA GPU and pulls several gigabytes of weights from Hugging Face on
the first run. `requirements.txt` pins the `+cu124` `torch` build with its index
URL deliberately — installing `diffusers` on top of an existing CUDA `torch`
without that pin replaces it with a CPU build, and the run then dies at the first
frame having already spent thirteen minutes downloading weights. That failure is
recorded in full in the comments of `requirements.txt` itself, which is where it
is useful.

Check the environment before spending a render on it:

```bash
.venv/Scripts/python -c "import torch; print(torch.__version__, torch.cuda.is_available())"
```

`2.6.0+cu124 True` is the answer. A bare version with `False` is the CPU trap.

## The check

```bash
python tools/sd-filter/test_sd_filter.py
```

Standard library plus `numpy`, no GPU and no weights. It is the one part of this
feature that can be gated at all, it runs in CI, and what it covers is described
on the canonical page.
