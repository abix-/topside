"""fit a photo's camera to the car's outline (reference.md, "Closing the
gap"): the model's outline drawn from the camera against the car's
outline cut out of the photo, thousands of edge pixels instead of a few
clicked points (the silhouette and render-and-compare methods: MegaPose,
"3D pose estimation by silhouette consistency").

The car is cut out of the photo with OpenCV's GrabCut, started from the
model drawn from the solved camera (inside it: likely car; well outside:
background). The score is how far apart the two outlines lie, in pixels,
both ways (each outline's pixels to the other's nearest), and how much of
each shape the other covers. The camera (turn and place, the lens kept)
is then moved to bring the outlines together, the clicked points kept as
a lighter pull.

    python scripts/mustang-gt/outline.py <body folder> <photo> [fit]   (needs numpy, opencv, scipy)

Writes overlays/<photo>_outline.jpg (the photo, the car's cut-out outline
in green, the model's in red) and, with fit, overlays/<photo>_outline_fit.jpg
and the fitted camera into overlays/outline_cameras.json.
"""

import json
import sys
from pathlib import Path

import cv2
import numpy as np
from scipy.optimize import minimize

HERE = Path(__file__).resolve().parent
BODY, PHOTO = Path(sys.argv[1]), sys.argv[2]
# fit: from the solved camera; search: from every view round the car first
# (the solved one may be the wrong one of two views that fit the clicked
# points alike, as 2354's was: MegaPose's coarse stage).
SEARCH = len(sys.argv) > 3 and sys.argv[3] == "search"
FIT = SEARCH or (len(sys.argv) > 3 and sys.argv[3] == "fit")
# Worked at this share of the photo's size (4 px of the photo to 1).
SCALE = 0.25
OUT = HERE / "overlays"

photo = cv2.imread(str(next(Path("C:/car").glob(f"*{PHOTO}.jpg"))))
# A photo with no solved camera (9184) starts at the 200 mm lens, 40 m off.
cam = json.loads((BODY / "cameras.json").read_text()).get(PHOTO) or {
    "focal": 46026.0, "size": [photo.shape[1], photo.shape[0]], "rot": np.eye(3).tolist(), "place": [0.0, 0.0, -40.0]}

# The wing is left out of the model's outline: the cut-outs mostly leave
# the thin blade out against the sky or grass, and it is fitted on its own.
# Its triangles: above the trunk, behind the rear glass.
def body_only(mesh):
    tri = mesh.reshape(-1, 3, 3)
    middle = tri.mean(axis=1)
    return tri[~((middle[:, 2] > 1.12) & (middle[:, 0] < -1.55))].reshape(-1, 3)


tri = body_only(np.load(BODY / "mesh.npy")).astype(np.float64)
small = cv2.resize(photo, None, fx=SCALE, fy=SCALE, interpolation=cv2.INTER_AREA)
height, width = small.shape[:2]
lens = np.array([[cam["focal"] * SCALE, 0, width / 2], [0, cam["focal"] * SCALE, height / 2], [0, 0, 1]])
rvec0, _ = cv2.Rodrigues(np.array(cam["rot"]))
tvec0 = -np.array(cam["rot"]) @ np.array(cam["place"])


def drawn(rvec, tvec, mesh=None):
    """the model's shape from this camera, as a mask."""
    rot, _ = cv2.Rodrigues(np.asarray(rvec, float))
    p = (tri if mesh is None else mesh) @ rot.T + tvec
    uv = (p[:, :2] / p[:, 2:3]) * lens[0, 0] + lens[:2, 2]
    mask = np.zeros((height, width), np.uint8)
    # One triangle at a time: filled together, overlapping ones cancel.
    for t in np.round(uv * 4).astype(np.int32).reshape(-1, 3, 2):
        cv2.fillConvexPoly(mask, t, 255, shift=2)
    return mask


def edge(mask):
    return cv2.morphologyEx(mask, cv2.MORPH_GRADIENT, np.ones((3, 3), np.uint8)) > 0


# The car cut out of the photo, started from a box round every point clicked
# on it (not from the model, so a wrong camera cannot shape the cut-out).
clicked = [v for k, v in json.loads((HERE / "refpoints.json").read_text()).get(PHOTO, {}).items() if k != "size"]
clicked += list(json.loads((HERE / "checkpoints.json").read_text()).get(PHOTO, {}).values())
lo, hi = np.min(clicked, axis=0) * SCALE, np.max(clicked, axis=0) * SCALE
pad = (hi - lo) * 0.12
x0, y0 = np.maximum(lo - pad, 1).astype(int)
x1, y1 = np.minimum(hi + pad, (width - 2, height - 2)).astype(int)
cut = HERE / "masks" / f"{PHOTO}.png"
if cut.exists():
    # Segment Anything's cut-out (cutout.py), far cleaner than GrabCut's.
    car = cv2.resize(cv2.imread(str(cut), cv2.IMREAD_GRAYSCALE), (width, height), interpolation=cv2.INTER_AREA)
    car = np.where(car > 127, 255, 0).astype(np.uint8)
else:
    seed = np.zeros((height, width), np.uint8)
    cv2.grabCut(small, seed, (x0, y0, x1 - x0, y1 - y0), np.zeros((1, 65)), np.zeros((1, 65)), 8, cv2.GC_INIT_WITH_RECT)
    car = np.where((seed == cv2.GC_FGD) | (seed == cv2.GC_PR_FGD), 255, 0).astype(np.uint8)
# Holes inside the car filled: only its outline counts.
outer, _ = cv2.findContours(car, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
car = np.zeros_like(car)
cv2.drawContours(car, outer, -1, 255, cv2.FILLED)
# Only the piece joined to the model's middle (not stray patches).
count, labels, stats, _ = cv2.connectedComponentsWithStats(car)
if count > 1:
    car = np.where(labels == 1 + np.argmax(stats[1:, cv2.CC_STAT_AREA]), 255, 0).astype(np.uint8)
car_edge = edge(car)
to_car_edge = cv2.distanceTransform((~car_edge).astype(np.uint8), cv2.DIST_L2, 5)

# The clicked points, as a lighter pull.
model = json.loads((BODY / "modelpoints.json").read_text())
placed = json.loads((BODY / "triangulated.json").read_text())
refs = json.loads((HERE / "refpoints.json").read_text()).get(PHOTO, {})
checks = json.loads((HERE / "checkpoints.json").read_text()).get(PHOTO, {})
# A body point measured from the photos (bundle.py's FREE) over the body's.
points = [(placed.get("body " + k) or model.get(k), v) for k, v in refs.items() if k != "size"]
points += [(placed.get(k), v) for k, v in checks.items()]
points = [(np.array(w, float), np.array(v, float) * SCALE) for w, v in points if w is not None]


# A pixel of either outline farther than this from the other counts as
# only this far (photo pixels): the cut-out misses parts (the thin wing
# against the grass) and takes in others (the car's shadow on the track),
# and those must not drag the camera.
CAP = 40.0
# An outline pixel this close to the other outline counts as on it.
NEAR = 10.0


def score(rvec, tvec):
    """the outlines' distance both ways (photo pixels, each capped at CAP),
    and each outline's share lying within NEAR of the other."""
    mask = drawn(rvec, tvec)
    mine = edge(mask)
    to_mine = cv2.distanceTransform((~mine).astype(np.uint8), cv2.DIST_L2, 5)
    d_mine = to_car_edge[mine] / SCALE
    d_car = to_mine[car_edge] / SCALE
    a = np.minimum(d_mine, CAP).mean()
    b = np.minimum(d_car, CAP).mean()
    return a, b, (d_mine <= NEAR).mean(), (d_car <= NEAR).mean(), mask


def spots(rvec, tvec):
    proj, _ = cv2.projectPoints(np.array([w for w, _ in points]), rvec, tvec, lens, None)
    return np.hypot(*(proj.reshape(-1, 2) - np.array([v for _, v in points])).T) / SCALE


def report(label, rvec, tvec):
    a, b, mine_near, car_near, mask = score(rvec, tvec)
    e = spots(rvec, tvec)
    print(f"{label}: outlines apart {a:.1f} px (model to car), {b:.1f} px (car to model); "
          f"within {NEAR:.0f} px: model outline {mine_near:.1%}, car outline {car_near:.1%}; "
          f"clicked points within 25 px {int((e <= 25).sum())} of {len(e)}")
    return mask


def picture(mask, name):
    view = small.copy()
    view[car_edge] = (0, 255, 0)
    view[edge(mask)] = (0, 0, 255)
    cv2.imwrite(str(OUT / name), view)


mask = report("as solved", rvec0, tvec0)
picture(mask, f"{PHOTO}_outline.jpg")

def looking(azimuth, elevation, roll, distance, target):
    """the camera `distance` off `target`, round the car by `azimuth` from
    the nose, up by `elevation`, turned by `roll` about its view."""
    a, e = np.radians(azimuth), np.radians(elevation)
    place = target + distance * np.array([np.cos(e) * np.cos(a), np.cos(e) * np.sin(a), np.sin(e)])
    forward = (target - place) / np.linalg.norm(target - place)
    right = np.cross(forward, (0.0, 0.0, 1.0))
    right /= np.linalg.norm(right)
    rot = np.array([right, np.cross(forward, right), forward])
    c, s = np.cos(np.radians(roll)), np.sin(np.radians(roll))
    rot = np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]]) @ rot
    return rot, -rot @ place


def placed_on_car(rot, tvec, mesh):
    """the camera moved nearer or farther and sideways so the model's shape
    is the cut-out's size and sits on its middle, and their overlap."""
    rvec, _ = cv2.Rodrigues(rot)
    car_m = cv2.moments(car, True)
    for _ in range(2):
        m = cv2.moments(drawn(rvec, tvec, mesh), True)
        if m["m00"] < 10:
            return rvec, tvec, 0.0
        tvec = tvec.copy()
        tvec[2] *= np.sqrt(m["m00"] / car_m["m00"])
        m = cv2.moments(drawn(rvec, tvec, mesh), True)
        du = car_m["m10"] / car_m["m00"] - m["m10"] / m["m00"]
        dv = car_m["m01"] / car_m["m00"] - m["m01"] / m["m00"]
        tvec[0] += du / lens[0, 0] * tvec[2]
        tvec[1] += dv / lens[0, 0] * tvec[2]
    mask = drawn(rvec, tvec, mesh)
    both = np.logical_and(mask > 0, car > 0).sum()
    return rvec.ravel(), tvec, both / np.logical_or(mask > 0, car > 0).sum()


def refined(rvec0, tvec0):
    def cost(x):
        rvec, tvec = rvec0 + x[:3] * 1e-3, tvec0 + x[3:] * 1e-2
        a, b, *_ = score(rvec, tvec)
        e = spots(rvec, tvec)
        # The outline in pixels both ways, the clicked points a tenth as
        # much each (robust, so a misread does not drag).
        return a + b + 0.1 * np.mean(np.sqrt(1 + (e / 10) ** 2) - 1) * 10

    fit = minimize(cost, np.zeros(6), method="Powell", options={"xtol": 1e-2, "ftol": 1e-3, "maxfev": 3000})
    return rvec0 + fit.x[:3] * 1e-3, tvec0 + fit.x[3:] * 1e-2, fit.fun


if SEARCH:
    small_mesh = body_only(np.load(BODY / "mesh_small.npy")).astype(np.float64)
    target = np.array([0.0, 0.0, 0.6])
    distance = max(np.linalg.norm(np.array(cam["place"]) - target), 20.0)
    tried = []
    for azimuth in range(0, 360, 10):
        for elevation in range(-5, 35, 5):
            for roll in range(-60, 70, 10):
                rot, tvec = looking(azimuth, elevation, roll, distance, target)
                rvec, tvec, overlap = placed_on_car(rot, tvec, small_mesh)
                tried.append((overlap, azimuth, elevation, roll, rvec, tvec))
    tried.sort(key=lambda t: -t[0])
    for overlap, azimuth, elevation, roll, *_ in tried[:5]:
        print(f"view round {azimuth} deg, up {elevation} deg, turned {roll} deg: overlap {overlap:.1%}")
    starts = [(t[4], t[5]) for t in tried[:5]] + [(rvec0.ravel(), tvec0)]
else:
    starts = [(rvec0.ravel(), tvec0)]

if FIT:
    rvec, tvec, _ = min((refined(r, t) for r, t in starts), key=lambda f: f[2])
    mask = report("fitted", rvec, tvec)
    picture(mask, f"{PHOTO}_outline_fit.jpg")
    rot, _ = cv2.Rodrigues(rvec)
    path = OUT / "outline_cameras.json"
    fitted = json.loads(path.read_text()) if path.exists() else {}
    fitted[PHOTO] = dict(cam, rot=rot.tolist(), place=(-rot.T @ tvec).tolist())
    path.write_text(json.dumps(fitted, indent=1))
