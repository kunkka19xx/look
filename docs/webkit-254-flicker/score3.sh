#!/usr/bin/env bash
cd "$(dirname "$0")"; b=ab-$1
ffmpeg -v error -i $b.mp4 -filter_complex "[0]split=3[a][c][d];[a]crop=300:150:800:540,format=gray,edgedetect,signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=$b.tile[o1];[c]crop=140:70:1130:465,format=gray,edgedetect,signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=$b.bt[o2];[d]crop=900:50:830:390,format=gray,edgedetect,signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=$b.bar[o3]" -map "[o1]" -f null - -map "[o2]" -f null - -map "[o3]" -f null -
python3 dips.py $b.tile $b.bt $b.bar | cut -c1-110
