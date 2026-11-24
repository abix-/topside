"""Join rustperf's findings to what each system costs in the running game.

rustperf (C:/code/rustperf) reads source only; the perf op measures the
running game. Each rustperf warning names the function it is in, and each
perf span names the system that ran, so the two join by name. The ranking
is by measured ms a step: a function with many findings that costs nothing
(it runs once, at load) ranks below one with few findings that runs every
step. Prior art: Factorio's show-time-usage, each entity type's update time
beside its count.

Usage:
  python scripts/perf_join.py <perf.json> <rustperf.log> [<rustperf.log> ...]

perf.json is the perf op's answer, read after the bench (saves/perf_bench.save:
pause, load, step 600 as a warm-up after a launch, load, step 600 twice, perf).
"""

import json
import re
import sys
from collections import defaultdict

# The crates the game is built from; a modforge workspace run reports every
# mod in it too.
CRATES = {"topside", "topside_content", "modforge"}


def findings(paths):
    """Every rustperf warning as (crate, function, file:line, what)."""
    out = []
    for path in paths:
        warning = loc = None
        with open(path, encoding="utf-8", errors="replace") as f:
            for line in f:
                if line.startswith("warning: ") and not line.startswith("warning: `"):
                    warning, loc = line[9:].strip(), None
                elif warning and loc is None and "--> " in line:
                    loc = line.split("--> ", 1)[1].strip()
                elif warning and "= note: in fn " in line:
                    func = line.split("= note: in fn ", 1)[1].strip()
                    # A path in topside's own tree starts at src; content's
                    # at content; modforge's run names its crates.
                    crate = re.split(r"[\\/]", loc or "?")[0]
                    crate = {"src": "topside", "content": "topside_content"}.get(crate, crate)
                    if crate in CRATES:
                        out.append((crate, func, loc, warning))
                    warning = None
    return out


def costs(perf_path):
    """Each span's ms a step (the perf op's frame is ms a frame), and the
    step's unit counts."""
    with open(perf_path, encoding="utf-8") as f:
        perf = json.load(f)
    per_frame = perf["steps_per_frame"] or 1.0
    spans = {}
    for entry in perf["frame"]:
        span = entry["span"]
        if span.startswith("system "):
            spans[span[7:]] = entry["ms"] / per_frame
    units = {c["what"][7:]: c["per_step"] for c in perf.get("counts", []) if c["what"].startswith("units: ")}
    return perf, spans, units


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        sys.exit(2)
    perf, spans, units = costs(sys.argv[1])
    by_func = defaultdict(list)
    for crate, func, loc, what in findings(sys.argv[2:]):
        by_func[(crate, func)].append((loc, what))
    rows = []
    for (crate, func), found in by_func.items():
        name = f"{crate.replace('-', '_')}::{func}"
        ms = spans.get(name, 0.0)
        rows.append((ms, len(found), name, found))
    rows.sort(key=lambda r: (-r[0], -r[1]))
    print(f"step {perf['step_ms']:.2f} ms (worst {perf['step_worst_ms']:.1f}), {perf['steps_per_frame']:.1f} steps a frame")
    print("units a step: " + ", ".join(f"{k} {v:.1f}" for k, v in units.items()))
    print("parts: " + ", ".join(f"{p['part']} {p['ms']:.2f}" for p in perf["parts"][:12]))
    print()
    print(f"{'ms/step':>8} {'found':>5}  function")
    for ms, n, name, found in rows:
        kinds = defaultdict(int)
        for _, what in found:
            kinds[what] += 1
        print(f"{ms:8.3f} {n:5}  {name}  [" + "; ".join(f"{k} x{v}" for k, v in sorted(kinds.items(), key=lambda kv: -kv[1])) + "]")
    print()
    print(f"{len(rows)} functions, {sum(r[1] for r in rows)} findings; measured ones first, the rest cost nothing a step in this bench")


if __name__ == "__main__":
    main()
