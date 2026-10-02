"""cut the car out of each photo with Segment Anything (Meta, 2023), for
fitting the cameras to the car's outline (outline.py; reference.md,
"Closing the gap"). The prompt is a box round every point clicked on the
car in that photo, and those points themselves as "this is the car". The
photo is cropped to the box first, so the model sees the car as large as
it can. Writes masks/<photo>.png (white: the car).

    python scripts/mustang-gt/cutout.py <sam checkpoint> [<photo> ...]   (needs torch, segment-anything, opencv)
"""

import json
import sys
from pathlib import Path

import cv2
import numpy as np
import torch
from segment_anything import SamPredictor, sam_model_registry

HERE = Path(__file__).resolve().parent
refs = json.loads((HERE / "refpoints.json").read_text())
checks = json.loads((HERE / "checkpoints.json").read_text())
photos = sys.argv[2:] or sorted(set(refs) | set(checks))
sam = sam_model_registry["vit_h"](checkpoint=sys.argv[1]).to("cuda" if torch.cuda.is_available() else "cpu")
predictor = SamPredictor(sam)
(HERE / "masks").mkdir(exist_ok=True)
for name in photos:
    path = next(Path("C:/car").glob(f"*{name}.jpg"), None)
    clicked = [v for k, v in refs.get(name, {}).items() if k != "size"] + list(checks.get(name, {}).values())
    if path is None or len(clicked) < 3:
        print(f"{name}: no photo or too few clicked points")
        continue
    photo = cv2.imread(str(path))
    height, width = photo.shape[:2]
    lo, hi = np.min(clicked, axis=0), np.max(clicked, axis=0)
    pad = (hi - lo) * 0.15
    x0, y0 = np.maximum(lo - pad, 0).astype(int)
    x1, y1 = np.minimum(hi + pad, (width, height)).astype(int)
    crop = cv2.cvtColor(photo[y0:y1, x0:x1], cv2.COLOR_BGR2RGB)
    predictor.set_image(crop)
    # The box alone, a little wider than the clicks (the car's outline lies
    # outside its clicked features): points on the car steer the model to
    # one part of it (the blue paint without the black hood, the wheels
    # or the splitter). Of its answers, the largest: the whole car.
    box = np.array([*(lo - pad / 2 - (x0, y0)), *(hi + pad / 2 - (x0, y0))])
    masks, scores, _ = predictor.predict(box=box, multimask_output=True)
    best = masks[int(np.argmax(masks.reshape(len(masks), -1).sum(axis=1)))]
    full = np.zeros((height, width), np.uint8)
    full[y0:y1, x0:x1] = best * 255
    cv2.imwrite(str(HERE / "masks" / f"{name}.png"), full)
    print(f"{name}: cut out, score {scores.max():.3f}, {best.mean():.1%} of the box")
