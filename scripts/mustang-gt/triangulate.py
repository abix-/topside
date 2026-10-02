"""find where our own parts truly are (reference.md, "How the model is
matched to the photos", step 6): each check point (checkpoints.json) seen
in two or more photos whose camera is solved is triangulated from them
(least squares over every photo that shows it). Prints its position in the
body's own frame (the frame modelpoints.json uses: metres, nose +x, the
car's left +y, up +z) and how far it lands from its reading in each photo.

    python scripts/mustang-gt/triangulate.py <body folder>   (needs numpy)
"""

import json
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
BODY = Path(sys.argv[1]) if len(sys.argv) > 1 else HERE
cameras = json.loads((BODY / "cameras.json").read_text())
checks = json.loads((HERE / "checkpoints.json").read_text())


def projection(cam):
    width, height = cam["size"]
    lens = np.array([[cam["focal"], 0, width / 2], [0, cam["focal"], height / 2], [0, 0, 1]])
    rot = np.array(cam["rot"])
    tvec = -rot @ np.array(cam["place"])
    return lens @ np.hstack([rot, tvec[:, None]])


seen = {}
for photo, points in checks.items():
    if photo not in cameras or cameras[photo]["error"] == float("inf"):
        continue
    for name, pixel in points.items():
        seen.setdefault(name, []).append((photo, pixel))

found = {}
for name, views in sorted(seen.items()):
    if len(views) < 2:
        print(f"{name:28s} only in {views[0][0]}: not placed")
        continue
    rows = []
    for photo, (u, v) in views:
        p = projection(cameras[photo])
        rows += [u * p[2] - p[0], v * p[2] - p[1]]
    _, _, vt = np.linalg.svd(np.array(rows))
    point = vt[-1][:3] / vt[-1][3]
    offs = []
    for photo, (u, v) in views:
        q = projection(cameras[photo]) @ np.append(point, 1.0)
        offs.append(f"{photo} {np.hypot(q[0] / q[2] - u, q[1] / q[2] - v):.0f}px")
    found[name] = point.round(4).tolist()
    print(f"{name:28s} {np.round(point, 3).tolist()}  off: {', '.join(offs)}")
(BODY / "triangulated.json").write_text(json.dumps(found, indent=1))
