#!/bin/bash
# Per-preset headless frame cost: no vsync, no present. Usage: bench-presets.sh [gpu-name] [WxH]
GPU="${1:-NVIDIA}"; SIZE="${2:-1920x1080}"; BIN=./target/release/ritmolux
PRESETS=("Nebula" "Leviathan" "Clifford" "Volute" "Dragon" "Ink on Paper" "Barnsley Fern" "Braid" "Murmuration" "Shatter")
"$BIN" --stream --sink stdout --gpu "$GPU" --size "$SIZE" --fps 240 --frames 1 --preset Nebula 2>&1 >/dev/null | grep '^renderer'
printf 'preset\trun1\trun2\trun3\tmedian_ms\tfps_equiv\n'
for p in "${PRESETS[@]}"; do
  r=()
  for i in 1 2 3; do
    r+=("$("$BIN" --stream --sink stdout --gpu "$GPU" --size "$SIZE" --fps 240 --frames 1440 --preset "$p" 2>&1 >/dev/null \
          | sed -nE 's/.*render\+readback ([0-9.]+) ms.*/\1/p' | tail -1)")
  done
  m=$(printf '%s\n' "${r[@]}" | sort -g | sed -n 2p)
  printf '%s\t%s\t%s\t%s\t%s\t%.0f\n' "$p" "${r[@]}" "$m" "$(awk -v m="$m" 'BEGIN{print 1000/m}')"
done
