#!/usr/bin/env bash
# usage: tools/capture.sh <label> [shows]
# Records <shows> summon cycles of the launcher at 60 fps and scores every frame
# for one-frame glitches. Same method as docs/webkit-254-flicker/ab.sh: crop a
# text region, edge-detect, and flag a frame whose sharpness drops below both
# neighbours. Wants wf-recorder, ffmpeg, socat and python3 on PATH (the dev
# shell has them) and a wlroots compositor.
set -u
cd "$(dirname "$0")/.."
LABEL=$1
SHOWS=${2:-8}
OUT=captures
BIN=${LOOK_GPUI_BIN:-../target/release-gpui/lookapp-gpui}
# Its own socket and no hotkey: the launcher in use keeps both.
SOCK=${XDG_RUNTIME_DIR:-/tmp}/look-gpui-probe.sock
export LOOK_PACE_PROBE=1 LOOK_CONTROL_SOCKET=$SOCK
# 1008x672 centred on a 2560x1440 output; override for another monitor.
GEOMETRY=${GEOMETRY:-"776,384 1008x672"}
# The recording is the window region, so crops are window relative. This is
# the clock tile, the panel's top-left 2x2 cell, caption and time included.
CROP=${CROP:-"crop=400:200:24:76"}

mkdir -p "$OUT"
"$BIN" --hidden >"$OUT/$LABEL.log" 2>&1 &
APP=$!
sleep 1.5
send() { printf '%s' "$1" | socat - "UNIX-CONNECT:$SOCK"; }

wf-recorder -r 60 -g "$GEOMETRY" -f "$OUT/$LABEL.mp4" >/dev/null 2>&1 &
REC=$!
sleep 1
for _ in $(seq 1 "$SHOWS"); do
    send show
    sleep 1.2
    send hide
    sleep 0.6
done
sleep 0.5
kill -INT $REC; wait $REC
send quit; wait $APP 2>/dev/null

ffmpeg -v error -i "$OUT/$LABEL.mp4" \
    -vf "$CROP,format=gray,edgedetect,signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=$OUT/$LABEL.edge" \
    -f null -
python3 tools/dips.py "$OUT/$LABEL.edge"
