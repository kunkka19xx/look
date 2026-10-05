#!/usr/bin/env bash
# usage: repro.sh <opaque|argb> ; repro window floats centered like Look (776,384 1008x672)
cd "$(dirname "$0")"; M=$1
export NIRI_SOCKET=$(ls /run/user/1000/niri.wayland-1.*.sock | head -1)
WR=/nix/store/fcb5r12yla0lcdwn0x22s4a1z4ajlbxz-wf-recorder-0.6.0/bin/wf-recorder
./repro/target/debug/repro $M >repro-$M.log 2>&1 & P=$!
sleep 1.2; niri msg action toggle-window-floating >/dev/null 2>&1; sleep 0.3; niri msg action center-window >/dev/null 2>&1; sleep 0.5
rm -f rp-$M.mp4; $WR -r 60 -f rp-$M.mp4 >/dev/null 2>&1 & W=$!
sleep 27; kill -INT $W; wait $W; kill $P; wait $P 2>/dev/null
ffmpeg -v error -i rp-$M.mp4 -filter_complex "[0]split[a][b];[a]crop=150:80:830:500,format=gray,edgedetect,signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=rp-$M.card[o1];[b]crop=900:30:810:430,format=gray,edgedetect,signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=rp-$M.bar[o2]" -map "[o1]" -f null - -map "[o2]" -f null -
python3 dips.py rp-$M.card rp-$M.bar
ffmpeg -v error -y -i rp-$M.mp4 -vf "select='eq(n\,400)',crop=1100:400:740:360,scale=550:-1" -frames:v 1 -fps_mode vfr rp-$M.png
