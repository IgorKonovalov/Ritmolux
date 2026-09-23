#!/bin/bash
# Live per-preset reading: each preset fullscreen on its own Hyprland workspace, 30 s, with music.
# Usage: live-presets.sh [gpu-name ...]   (default: NVIDIA AMD)
# The pseudo-name `default` runs unflagged, so the row reads whatever adapter the app itself picks.
# Needs Hyprland 0.56+ (Lua dispatch), jq, a release build, and audio playing.
# The workspace is FOCUSED, not silent: a hidden workspace gets no frame callbacks, so the
# present would stall and the reading would measure the compositor, not the preset.
cd "$(git rev-parse --show-toplevel)" || exit 1
BIN=$PWD/target/release/ritmolux
LOG=~/.local/share/Ritmolux/diagnostics.log
WS=9
if [ $# -gt 0 ]; then GPUS=("$@"); else GPUS=(NVIDIA AMD); fi
PRESETS=("Nebula" "Leviathan" "Clifford" "Volute" "Dragon" "Ink on Paper" "Barnsley Fern" "Braid" "Murmuration" "Shatter")
HOME_WS=$(hyprctl activeworkspace -j | jq .id)
# SIGTERM, not SIGINT: a job started with & in a non-interactive shell ignores SIGINT.
trap 'pkill -TERM -x ritmolux; hyprctl dispatch "hl.dsp.focus({workspace = $HOME_WS})" >/dev/null' EXIT

printf 'gpu\tpreset\twindow\tsamples\tfps_med\tfps_min\tavg_ms_med\tp99_ms_med\tp99_ms_max\tdropped\tsamples_under_60\n'
for gpu in "${GPUS[@]}"; do
  for p in "${PRESETS[@]}"; do
    before=$(wc -l < "$LOG")
    hyprctl dispatch "hl.dsp.focus({workspace = $WS})" >/dev/null
    if [ "$gpu" = default ]; then pin=(); else pin=(--gpu "$gpu"); fi
    "$BIN" "${pin[@]}" --preset "$p" >/dev/null 2>&1 &
    pid=$!
    sleep 3; hyprctl dispatch 'hl.dsp.window.fullscreen()' >/dev/null
    sleep 9
    win=$(hyprctl clients -j | jq -r ".[] | select(.pid==$pid) | \"\(.size[0])x\(.size[1]) fs\(.fullscreen)\"" | head -1)
    sleep 20; kill -TERM $pid; while kill -0 $pid 2>/dev/null; do sleep 0.2; done; sleep 1
    tail -n +$((before+1)) "$LOG" | grep '^# renderer' >&2
    # The first 6 one-second rows cover startup and the fullscreen resize; they are dropped.
    tail -n +$((before+1)) "$LOG" | grep -v '^#' | tail -n +7 | awk -v p="$p" -v g="$gpu" -v s="$win" '
      {f[NR]=$2; a[NR]=$3; q[NR]=$4; if(NR==1)d0=$6; d1=$6; if($2<60)u++}
      END{n=NR; asort(f); asort(a); asort(q); m=int(n/2)+1;
        printf "%s\t%s\t%s\t%d\t%.1f\t%.1f\t%.2f\t%.2f\t%.2f\t%d\t%d\n", g,p,s,n,f[m],f[1],a[m],q[m],q[n],d1-d0,u}'
  done
done
