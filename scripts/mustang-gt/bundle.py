"""refine every photo's camera and the true places of our own parts
together (reference.md, "How the model is matched to the photos", step 5:
bundle adjustment).

Unknowns: each solved camera's place and turn (the lens stays as solved),
and the 3D place of each check point (checkpoints.json) seen in two or
more photos. Known: the body's reference points on the model
(modelpoints.json). Every reading in every photo pulls on them at once, so
a camera with few body points of its own is held by the parts it shares
with the others. A robust loss keeps a misread from dragging the rest.

Writes cameras.json and triangulated.json back into the body's folder,
and prints each photo's error and the readings still off.

    python scripts/mustang-gt/bundle.py <body folder>   (needs numpy, opencv, scipy)
"""

import json
import sys
from pathlib import Path

import cv2
import numpy as np
from scipy.optimize import least_squares

HERE = Path(__file__).resolve().parent
BODY = Path(sys.argv[1]) if len(sys.argv) > 1 else HERE
# The same points left out of fitting as in cameras.py.
NOT_FITTED = ("front hub", "rear hub", "front arch", "rear arch", "reversing light", "windshield")
# Reference points where the downloaded body's shape differs from the car:
# their true place is solved from the photos like our own parts, not taken
# from the body, and written as "body <name>" for warping the body to.
# Tried for the nose (pony, top led bar front, grille top corner, grille
# outer point, amber tip): the whole nose came out shifted together, from
# the front cameras, not the body's shape (reference.md, "The nose,
# measured"). Empty until the front cameras are held by more points.
# The wheels are always measured (reference.md, "Ride height"); others can
# be set free by name.
FREE = ("front hub", "rear hub", "front arch", "rear arch") + (tuple(sys.argv[2].split(",")) if len(sys.argv) > 2 else ())
# How far (pixels) a reading may be off before the loss stops trusting it.
SCALE = 10.0
OFF = 25.0

cameras = json.loads((BODY / "cameras.json").read_text())
model = json.loads((BODY / "modelpoints.json").read_text())
refs = json.loads((HERE / "refpoints.json").read_text())
checks = json.loads((HERE / "checkpoints.json").read_text())
photos = [p for p, c in cameras.items() if np.isfinite(c["error"])]

# Every reading: (photo, point name, pixel, known 3D or None).
readings = []
for photo in photos:
    for name, px in refs.get(photo, {}).items():
        if name == "size" or name not in model:
            continue
        # A point set free is solved from the photos even when it is not
        # fitted (the wheels: measured, never fitted to).
        if FREE and name.startswith(FREE):
            readings.append((photo, f"body {name}", px, None))
            continue
        if name.startswith(NOT_FITTED):
            continue
        readings.append((photo, name, px, model[name]))
    for name, px in checks.get(photo, {}).items():
        readings.append((photo, name, px, None))
counts = {}
for photo, name, _, known in readings:
    if known is None:
        counts[name] = counts.get(name, 0) + 1
unknown = sorted(n for n, c in counts.items() if c >= 2)
readings = [r for r in readings if r[3] is not None or r[1] in unknown]


def lens_of(cam):
    width, height = cam["size"]
    return np.array([[cam["focal"], 0, width / 2], [0, cam["focal"], height / 2], [0, 0, 1]])


# Starting values: the cameras as solved alone, the parts triangulated.
start = []
for photo in photos:
    rot = np.array(cameras[photo]["rot"])
    rvec, _ = cv2.Rodrigues(rot)
    start += list(rvec.ravel()) + list((-rot @ np.array(cameras[photo]["place"])).ravel())
tri_path = BODY / "triangulated.json"
tri = json.loads(tri_path.read_text()) if tri_path.exists() else {}
for name in unknown:
    # A body point starts where the body has it.
    body_start = model.get(name.removeprefix("body ")) if name.startswith("body ") else None
    start += tri.get(name, body_start or [0.0, 0.0, 1.0])
start = np.array(start)
cam_index = {p: i for i, p in enumerate(photos)}
point_index = {n: i for i, n in enumerate(unknown)}


def residuals(params):
    out = []
    for photo, name, px, known in readings:
        i = cam_index[photo] * 6
        rvec, tvec = params[i : i + 3], params[i + 3 : i + 6]
        if known is None:
            j = len(photos) * 6 + point_index[name] * 3
            where = params[j : j + 3]
        else:
            where = np.array(known)
        proj, _ = cv2.projectPoints(np.array([where], dtype=np.float64), rvec, tvec, lens_of(cameras[photo]), None)
        out += list(proj.ravel() - np.array(px, dtype=np.float64))
    return np.array(out)


before = np.hypot(*residuals(start).reshape(-1, 2).T)
fit = least_squares(residuals, start, loss="soft_l1", f_scale=SCALE, x_scale="jac")
after = np.hypot(*fit.fun.reshape(-1, 2).T)
print(f"readings {len(readings)}, cameras {len(photos)}, parts placed {len(unknown)}")
print(f"agreeing within {OFF:.0f} px: before {int((before <= OFF).sum())}, after {int((after <= OFF).sum())}")

refined = {}
print(f"{'photo':6s} {'agree':>5s} {'of':>3s} {'error px':>8s}  off (px)")
for photo in photos:
    i = cam_index[photo] * 6
    rot, _ = cv2.Rodrigues(fit.x[i : i + 3])
    tvec = fit.x[i + 3 : i + 6]
    mine = [(r[1], e) for r, e in zip(readings, after) if r[0] == photo]
    good = [e for _, e in mine if e <= OFF]
    rms = float(np.sqrt(np.mean(np.square(good)))) if len(good) >= 4 else float("inf")
    off = ", ".join(f"{n} {e:.0f}" for n, e in mine if e > OFF)
    print(f"{photo:6s} {len(good):5d} {len(mine):3d} {rms:8.1f}  {off}")
    refined[photo] = dict(cameras[photo], rot=rot.tolist(), place=(-rot.T @ tvec).tolist(), error=rms)
for photo, cam in cameras.items():
    refined.setdefault(photo, cam)
# How sure each placed part is (the standard error of a least squares
# fit: the inverse of J^T J times the readings' spread): what pixel agreement
# cannot show, a part seen only along its view line agreeing well in every
# photo and still free to slide metres along it. In cm along the car,
# across it and up.
spread = float(np.sqrt(np.mean(np.square(after[after <= OFF]))))
covariance = np.linalg.pinv(fit.jac.T @ fit.jac) * spread**2
sure = {}
placed = {}
print(f"reading spread {spread:.1f} px; each part, and how far it could be off (cm, 1 sigma): along, across, up")
for name in unknown:
    j = len(photos) * 6 + point_index[name] * 3
    placed[name] = fit.x[j : j + 3].round(4).tolist()
    sure[name] = (np.sqrt(np.diag(covariance)[j : j + 3]) * 100).round(1).tolist()
    print(f"{name:28s} {np.round(fit.x[j : j + 3], 3).tolist()}  +- {sure[name]}")
(BODY / "cameras.json").write_text(json.dumps(refined, indent=1))
tri_path.write_text(json.dumps(placed, indent=1))
(BODY / "uncertainty.json").write_text(json.dumps(sure, indent=1))
