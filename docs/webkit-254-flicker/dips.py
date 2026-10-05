import sys
def series(p):
    return [float(l.split('=')[1]) for l in open(p) if 'YAVG=' in l]
for p in sys.argv[1:]:
    v = series(p); hits = []
    for i in range(1, len(v)-1):
        a, c = v[i-1], v[i+1]
        if abs(a-c) < 0.15*max(a, c) and v[i] < 0.75*a and v[i] < 0.75*c:
            hits.append(f"{i}:{v[i]/max(a,c):.2f}")
    print(f"{p.split('/')[-1]:18s} dips={len(hits):2d}  {' '.join(hits)}")
