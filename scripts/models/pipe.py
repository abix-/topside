"""the pipe: a metre of rusty steel with a taped grip at one end.

Usage:
    blender -b --python scripts/models/pipe.py -- assets/models/pipe.glb
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from common import box, clear, cylinder, export, join, material, out_path  # noqa: E402

LENGTH = 1.0
RADIUS = 0.025

clear()
steel = material("steel", (0.45, 0.42, 0.40))
tape = material("tape", (0.18, 0.16, 0.14))

parts = [
    cylinder("shaft", RADIUS, LENGTH, sides=8, mat=steel),
    # The grip: a fatter band over the near end.
    cylinder("grip", RADIUS * 1.25, 0.22, sides=8, at=(-LENGTH / 2 + 0.14, 0, 0), mat=tape),
    # A threaded collar at the far end, so the hitting end reads.
    cylinder("collar", RADIUS * 1.4, 0.05, sides=8, at=(LENGTH / 2 - 0.04, 0, 0), mat=steel),
    # A dent: a shallow box pressed into the shaft a third of the way up.
    box("dent", (0.06, RADIUS * 1.1, RADIUS * 0.5), at=(0.1, 0, RADIUS * 0.9), mat=steel),
]
pipe = join(parts, "pipe")
export(pipe, out_path())
