"""Flag frames whose edge sharpness dips below both neighbours.

Same detector as docs/webkit-254-flicker/dips.py. A hit is an isolated frame,
not a fade: both neighbours must agree with each other within 15 percent and
the frame must sit under 75 percent of them.
"""
import sys


def series(path):
    return [float(l.split("=")[1]) for l in open(path) if "YAVG=" in l]


for path in sys.argv[1:]:
    v = series(path)
    hits = []
    for i in range(1, len(v) - 1):
        a, c = v[i - 1], v[i + 1]
        if abs(a - c) < 0.15 * max(a, c) and v[i] < 0.75 * a and v[i] < 0.75 * c:
            hits.append(f"{i}:{v[i] / max(a, c):.2f}")
    print(f"{path}: {len(hits)} dips in {len(v)} frames  {' '.join(hits)}")
