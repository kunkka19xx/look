#!/usr/bin/env bash
# usage: ab.sh <label> [cycles]
set -u
cd "$(dirname "$0")"
L=$1; N=${2:-8}
BIN=/home/kunkka/Documents/git/look/apps/linows/src-tauri/target/debug/lookapp
WR=/nix/store/fcb5r12yla0lcdwn0x22s4a1z4ajlbxz-wf-recorder-0.6.0/bin/wf-recorder
export NIRI_SOCKET=$(ls /run/user/1000/niri.wayland-1.*.sock | head -1)
WAYLAND_DEBUG="${WLDBG:-}" LOOK_WEBKIT_RESTART="${RESTART:-}" LOOK_WEBKIT_FEATURES="${FEAT:-}" LD_LIBRARY_PATH=/nix/store/51i6f4gci385q1jrmzdmiy9ijhhz6s6g-gtk-layer-shell-0.10.1/lib LOOK_CONFIG_PATH=$PWD/ab.config $BIN >dev-$L.log 2>&1 &
P=$!
sleep 6
U=$(busctl --user list --no-legend | awk -v p=$P '$1 ~ /^:/ && $2==p {print $1}' | while read u; do busctl --user introspect $u /com/look/Desktop 2>/dev/null | grep -q "^com.look.Desktop " && echo $u; done | head -1)
[ -z "$U" ] && { echo "no bus conn for $P"; kill $P; exit 1; }
T() { dbus-send --session --type=method_call --dest=$U /com/look/Desktop com.look.Desktop.Toggle; }
# ensure hidden start: dev app may start shown; probe visibility not available, so do one hide/show pair first
rm -f ab-$L.mp4
$WR -r 60 -f ab-$L.mp4 >/dev/null 2>&1 &
W=$!
sleep 1.5
for i in $(seq 1 $N); do T; sleep 0.6; T; sleep 1.6; done
sleep 0.5
kill -INT $W; wait $W
kill $P; wait $P 2>/dev/null
ffmpeg -v error -i ab-$L.mp4 -vf "crop=300:150:800:540,format=gray,edgedetect,signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=ab-$L.edge" -f null -
grep -o 'YAVG=.*' ab-$L.edge | cut -d= -f2 | awk '{v[NR-1]=$1; if($1>mx)mx=$1} END{n=0; for(i=1;i<NR-1;i++){a=v[i-1];b=v[i];c=v[i+1]; if(a>0.3*mx && c>0.3*mx && b<0.6*a && b<0.6*c){n++; printf "dip %d %.2f %.2f %.2f\n",i,a,b,c}} printf "'"$L"': %d glitch frames in %d frames (max %.2f)\n", n, NR, mx}'
