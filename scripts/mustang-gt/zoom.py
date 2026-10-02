"""a part of a photo, enlarged, with a pixel grid, for reading points off
it (reference.md, "How the model is matched to the photos").

    python scripts/mustang-gt/zoom.py <out.jpg> <photo> <x> <y> <width> <height> [<photo> <x> <y> <width> <height> ...]   (needs opencv)

Each piece is scaled to 900 pixels across; pieces sit side by side.
Readings in checkpoints.json are marked green.
"""

import json
import sys
from pathlib import Path

import cv2
import numpy as np

HERE = Path(__file__).resolve().parent
checks = json.loads((HERE / "checkpoints.json").read_text())
out, args = sys.argv[1], sys.argv[2:]
pieces = []
for i in range(0, len(args), 5):
    name, x0, y0, w, h = args[i], *(int(a) for a in args[i + 1 : i + 5])
    im = cv2.imread(str(next(Path("C:/car").glob(f"*{name}.jpg"))))
    for v in checks.get(name, {}).values():
        cv2.circle(im, tuple(v), 5, (0, 255, 0), 2)
    k = 900 / w
    c = cv2.resize(im[y0 : y0 + h, x0 : x0 + w], None, fx=k, fy=k, interpolation=cv2.INTER_CUBIC)
    # Fine lines about every 30 screen pixels, labelled lines five times that.
    step = next(s for s in (5, 10, 20, 25, 50, 100, 200) if s * k >= 30)
    for x in range((x0 // step + 1) * step, x0 + w, step):
        X = int((x - x0) * k)
        big = x % (step * 5) == 0
        cv2.line(c, (X, 0), (X, c.shape[0]), (0, 255, 255) if big else (0, 90, 90), 1)
        if big:
            cv2.putText(c, str(x), (X + 3, 20), 0, 0.6, (0, 255, 255), 2)
    for y in range((y0 // step + 1) * step, y0 + h, step):
        Y = int((y - y0) * k)
        big = y % (step * 5) == 0
        cv2.line(c, (0, Y), (c.shape[1], Y), (0, 255, 255) if big else (0, 90, 90), 1)
        if big:
            cv2.putText(c, str(y), (3, Y - 5), 0, 0.6, (0, 255, 255), 2)
    cv2.putText(c, name, (c.shape[1] - 90, c.shape[0] - 12), 0, 0.9, (0, 0, 255), 2)
    pieces.append(c)
height = max(p.shape[0] for p in pieces)
cv2.imwrite(out, np.hstack([cv2.copyMakeBorder(p, 0, height - p.shape[0], 0, 0, cv2.BORDER_CONSTANT) for p in pieces]))
