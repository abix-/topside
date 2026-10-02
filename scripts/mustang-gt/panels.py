"""for each photo: the photo, our model from the photo's solved camera, and
the two laid over each other, side by side, cropped to the car, into
scripts/mustang-gt/compare/<photo>.jpg.

Takes the folder holding overlay.py's output (<name>.png and
<name>.render.png for each photo, named all_<photo>).

    python scripts/mustang-gt/panels.py <overlay folder>
"""

import sys
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
SOURCE = Path(sys.argv[1])
OUT = HERE / "compare"
OUT.mkdir(exist_ok=True)
# Only these photos, if named after the folder.
ONLY = sys.argv[2:]
for render in sorted(SOURCE.glob("all_*.render.png")):
    name = render.name.split("_")[1].split(".")[0]
    if ONLY and name not in ONLY:
        continue
    photo = Image.open(next(Path("C:/car").glob(f"*{name}.jpg"))).convert("RGB")
    model = Image.open(render).convert("RGBA")
    over = Image.open(SOURCE / f"all_{name}.png").convert("RGB")
    # Crop to where the car is: the model's own outline, with room round it.
    box = model.getchannel("A").getbbox()
    pad = 250
    box = (max(box[0] - pad, 0), max(box[1] - pad, 0), min(box[2] + pad, photo.width), min(box[3] + pad, photo.height))
    grey = Image.new("RGB", model.size, (110, 115, 120))
    grey.paste(model, (0, 0), model)
    panels = [p.crop(box) for p in (photo, grey, over)]
    h = 900
    panels = [p.resize((round(p.width * h / p.height), h)) for p in panels]
    sheet = Image.new("RGB", (sum(p.width for p in panels) + 40, h), (0, 0, 0))
    x = 0
    for p in panels:
        sheet.paste(p, (x, 0))
        x += p.width + 20
    sheet.save(OUT / f"{name}.jpg", quality=88)
    print(OUT / f"{name}.jpg")
