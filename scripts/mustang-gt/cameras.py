"""solve where each photo's camera stood (reference.md, "How the model is
matched to the photos", step 4).

For every photo in refpoints.json: the reference points it shows, matched
to the same points on the model (modelpoints.json), give the camera's
place and turn (OpenCV solvePnPRansac). The lens's focal length in pixels
is not known (the photos are cropped and resized from the camera's own
size), so it is searched for: the one whose fit leaves the smallest error.
Points more than OUTLIER pixels from where the fit puts them are flagged,
to be read again. Writes cameras.json and prints each photo's error.

    python scripts/mustang-gt/cameras.py      (needs numpy and opencv)
"""

import json
from pathlib import Path

import cv2
import numpy as np

HERE = Path(__file__).resolve().parent
# A point further than this from where the fit puts it does not agree with
# the others (the readings are good to about 5 to 15 pixels).
OUTLIER = 20.0
# Points never used to fit a camera: the wheels and arches move with the
# suspension, the reversing light is not the same spot on the model, and
# the windshield's corners are read where the car's black roof band (paint,
# not the body's shape) meets the pillar.
NOT_FITTED = ("front hub", "rear hub", "front arch", "rear arch", "reversing light", "windshield")

import sys

# The model's points: another body's points file can be given (a candidate).
model = json.loads(Path(sys.argv[3]).read_text() if len(sys.argv) > 3 else (HERE / "modelpoints.json").read_text())
photos = json.loads((HERE / "refpoints.json").read_text())
# A test of the model's width: its points spread across the car by this
# much (1 = as built).
WIDEN = float(sys.argv[1]) if len(sys.argv) > 1 else 1.0
model = {k: [x, y * WIDEN, z] for k, (x, y, z) in model.items()}


def fit(world, image, focal, size):
    """the camera for one focal length: its turn, place, the points that
    agree and the error of each point. Fitted to all the points at once,
    then the worst point dropped while it is off by more than OUTLIER, and
    fitted again (iterative outlier rejection). Not RANSAC: from 30 to 40 m
    with a long lens a camera from 4 points swings far on a few pixels of
    reading error, so its guesses are useless."""
    width, height = size
    lens = np.array([[focal, 0, width / 2], [0, focal, height / 2], [0, 0, 1]], dtype=np.float64)
    keep = np.arange(len(world))
    while True:
        try:
            ok, rvec, tvec = cv2.solvePnP(world[keep], image[keep], lens, None, flags=cv2.SOLVEPNP_SQPNP)
            ok, rvec, tvec = cv2.solvePnP(world[keep], image[keep], lens, None, rvec, tvec, True, cv2.SOLVEPNP_ITERATIVE)
        except cv2.error:
            return None
        proj, _ = cv2.projectPoints(world, rvec, tvec, lens, None)
        err = np.linalg.norm(proj.reshape(-1, 2) - image, axis=1)
        worst = keep[np.argmax(err[keep])]
        if err[worst] <= OUTLIER or len(keep) <= 6:
            break
        keep = keep[keep != worst]
    inl = err <= OUTLIER
    rms = float(np.sqrt((err[inl] ** 2).mean())) if inl.sum() >= 4 else float("inf")
    return rms, int(inl.sum()), rvec, tvec, err, lens


# The zoom each photo was taken at (its EXIF focal length, mm). All are
# cropped and resized alike (4800 by 3771), so one focal length in pixels
# at 200 mm, scaled by each photo's zoom, fits them all.
ZOOM = {"9131": 135.0, "2428": 195.0}


def photo_points(seen):
    keys = [k for k in seen if k != "size" and k in model and not k.startswith(NOT_FITTED)]
    world = np.array([model[k] for k in keys], dtype=np.float64)
    image = np.array([seen[k] for k in keys], dtype=np.float64)
    return keys, world, image, seen.get("size", [4800, 3771])


def fit_all(at_200):
    """every photo's camera at this focal length at 200 mm, and the total:
    points agreeing, then error."""
    fits, agree, sq = {}, 0, 0.0
    for name, seen in photos.items():
        keys, world, image, size = photo_points(seen)
        if len(keys) < 4:
            continue
        r = fit(world, image, at_200 * ZOOM.get(name, 200.0) / 200.0, size)
        if r:
            fits[name] = r
            agree += r[1]
            sq += r[0] ** 2 * r[1]
    return agree, -sq, fits


# The lens, from the photos' own data: the camera's focal plane resolution
# (230.13 pixels a millimetre, the D850's 8256 pixels across its 35.9 mm
# sensor) times 200 mm, if the photos are crops of the camera's pixels
# and not resized. Given on the command line, that value is used; else it
# is searched for: most points agreeing across all the photos first, then
# the smallest error.
if len(sys.argv) > 2:
    best_focal = float(sys.argv[2])
else:
    best_focal = max(np.geomspace(3000, 300000, 200), key=lambda f: fit_all(f)[:2])
_, _, fits = fit_all(best_focal)
print(f"focal length at 200 mm: {best_focal:.0f} px")

cameras = {}
print(f"{'photo':6s} {'points':>6s} {'agree':>5s} {'error px':>8s} {'focal px':>9s}  flagged (px off)")
for name, seen in photos.items():
    keys, world, image, size = photo_points(seen)
    if name not in fits:
        print(f"{name:6s} {len(keys):6d}  no fit")
        continue
    rms, agree, rvec, tvec, err, lens = fits[name]
    focal = float(lens[0, 0])
    rot, _ = cv2.Rodrigues(rvec)
    flagged = ", ".join(f"{k} {e:.0f}" for k, e in zip(keys, err) if e > OUTLIER)
    print(f"{name:6s} {len(keys):6d} {agree:5d} {rms:8.1f} {focal:9.0f}  {flagged}")
    cameras[name] = {
        "rot": rot.tolist(),
        "place": (-rot.T @ tvec).ravel().tolist(),
        "focal": focal,
        "size": size,
        "error": rms,
        "errors": {k: round(float(e), 1) for k, e in zip(keys, err)},
    }
(Path(sys.argv[3]).parent if len(sys.argv) > 3 else HERE).joinpath("cameras.json").write_text(json.dumps(cameras, indent=1))
