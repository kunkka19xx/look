#!/usr/bin/env bash
# usage: tools/pace.sh <label> [seconds]
# Shows the launcher with the pacing probe on, then prints frame-delta stats for
# the steady state (everything after the first second).
set -u
cd "$(dirname "$0")/.."
LABEL=$1
SECS=${2:-4}
OUT=captures
BIN=${LOOK_GPUI_BIN:-../target/release/lookapp-gpui}
# Its own socket and no hotkey: the launcher in use keeps both.
SOCK=${XDG_RUNTIME_DIR:-/tmp}/look-gpui-probe.sock
export LOOK_PACE_PROBE=1 LOOK_CONTROL_SOCKET=$SOCK
mkdir -p "$OUT"
"$BIN" --hidden >"$OUT/pace-$LABEL.log" 2>&1 &
APP=$!
sleep 1.5
printf show | socat - "UNIX-CONNECT:$SOCK"
sleep "$SECS"
printf quit | socat - "UNIX-CONNECT:$SOCK"
wait $APP 2>/dev/null
grep "^\[look" "$OUT/pace-$LABEL.log" || true
python3 - "$OUT/pace-$LABEL.log" "$LABEL" <<'PY'
import re, sys
ts = [int(m.group(1)) for l in open(sys.argv[1]) if (m := re.match(r"probe (\d+)", l))]
d = [(ts[i + 1] - ts[i]) / 1000 for i in range(len(ts) - 1)]
steady = [x for i, x in enumerate(d) if ts[i] - ts[0] > 1_000_000]
if not steady:
    sys.exit("no steady-state frames")
steady.sort()
mean = sum(steady) / len(steady)
p50 = steady[len(steady) // 2]
p90 = steady[int(len(steady) * 0.9)]
print(f"{sys.argv[2]}: {len(steady)} frames, mean {mean:.1f} ms, p50 {p50:.0f}, p90 {p90:.0f}, max {steady[-1]:.0f}  (~{1000/mean:.0f} fps)")
PY
