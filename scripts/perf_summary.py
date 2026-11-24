"""Print the perf op's step, worst step and costliest parts from a saved perf answer.

Usage: python scripts/perf_summary.py <perf.json> [count]
"""

import json
import sys


def main() -> None:
    path = sys.argv[1]
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 12
    with open(path, encoding="utf8") as f:
        perf = json.load(f)
    print(f"step {perf['step_ms']:.2f} ms  worst {perf['step_worst_ms']:.2f} ms at tick {perf.get('step_worst_tick')}  steps a frame {perf['steps_per_frame']:.1f}")
    over = perf.get("over_budget_steps") or []
    print(f"over budget: {perf.get('over_budget')}  " + " ".join(f"{s['tick']}:{s['ms']:.1f}" for s in over))
    print("by worst:")
    for part in sorted(perf["parts"], key=lambda p: -(p["worst_ms"] or 0))[:count]:
        print(f"  {part['part']:<32} {part['ms']:7.2f} {part['worst_ms'] or 0:8.2f}")
    print("by mean:")
    for part in sorted(perf["parts"], key=lambda p: -p["ms"])[:count]:
        print(f"  {part['part']:<32} {part['ms']:7.2f} {part['worst_ms'] or 0:8.2f}")


if __name__ == "__main__":
    main()
