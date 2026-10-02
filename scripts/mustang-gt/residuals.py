"""how far each measured part (triangulated.json) lands from its reading
(checkpoints.json) in each photo, from the solved cameras. Only names
containing the given words.

    python scripts/mustang-gt/residuals.py <body folder> <word> [<word> ...]   (needs numpy, opencv)
"""

import json
import sys
from pathlib import Path

import cv2
import numpy as np

HERE = Path(__file__).resolve().parent
body, words = Path(sys.argv[1]), sys.argv[2:]
cameras = json.loads((body / "cameras.json").read_text())
placed = json.loads((body / "triangulated.json").read_text())
checks = json.loads((HERE / "checkpoints.json").read_text())
for name in sorted(placed):
    if not any(w in name for w in words):
        continue
    offs = []
    for photo, points in checks.items():
        if name not in points or photo not in cameras:
            continue
        c = cameras[photo]
        w, h = c["size"]
        lens = np.array([[c["focal"], 0, w / 2], [0, c["focal"], h / 2], [0, 0, 1]])
        rot = np.array(c["rot"])
        rvec, _ = cv2.Rodrigues(rot)
        tvec = -rot @ np.array(c["place"])
        u, v = cv2.projectPoints(np.array([placed[name]], dtype=np.float64), rvec, tvec, lens, None)[0].ravel()
        offs.append(f"{photo} {np.hypot(u - points[name][0], v - points[name][1]):.0f}px")
    print(f"{name:30s} {np.round(placed[name], 3).tolist()}  {', '.join(offs)}")
