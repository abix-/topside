"""draw, on a photo, where each reference point was read (green) and where
the model's same point lands from the photo's solved camera (red), joined
by a line and named, to see which side picked the wrong spot.

    python scripts/mustang-gt/check.py <photo> <out.png>   (needs opencv)
"""

import json
import sys
from pathlib import Path

import cv2
import numpy as np

HERE = Path(__file__).resolve().parent
PHOTOS = Path("C:/car")
name, out = sys.argv[1], sys.argv[2]
# Another body's folder (with its cameras.json and modelpoints.json) can
# be given; by default the files here.
BODY = Path(sys.argv[3]) if len(sys.argv) > 3 else HERE
camera = json.loads((BODY / "cameras.json").read_text())[name]
model = json.loads((BODY / "modelpoints.json").read_text())
seen = json.loads((HERE / "refpoints.json").read_text())[name]
photo = cv2.imread(str(next(PHOTOS.glob(f"*{name}.jpg"))))

rot = np.array(camera["rot"])
tvec = -rot @ np.array(camera["place"])
width, height = camera["size"]
lens = np.array([[camera["focal"], 0, width / 2], [0, camera["focal"], height / 2], [0, 0, 1]])
rvec, _ = cv2.Rodrigues(rot)
xs, ys = [], []
for key, (px, py) in ((k, v) for k, v in seen.items() if k != "size" and k in model):
    (mx, my), = cv2.projectPoints(np.array([model[key]], dtype=np.float64), rvec, tvec, lens, None)[0].reshape(-1, 2)
    off = ((mx - px) ** 2 + (my - py) ** 2) ** 0.5
    colour = (0, 200, 0) if off <= 20 else (0, 0, 255)
    cv2.line(photo, (int(px), int(py)), (int(mx), int(my)), (0, 255, 255), 3)
    cv2.circle(photo, (int(px), int(py)), 14, (0, 255, 0), 4)
    cv2.drawMarker(photo, (int(mx), int(my)), (0, 0, 255), cv2.MARKER_CROSS, 30, 4)
    cv2.putText(photo, f"{key} {off:.0f}", (int(px) + 16, int(py) - 10), cv2.FONT_HERSHEY_SIMPLEX, 1.1, colour, 3)
    xs += [px, mx]
    ys += [py, my]
# The whole photo at full size, for cropping close.
cv2.imwrite(out, photo)
print(out)
