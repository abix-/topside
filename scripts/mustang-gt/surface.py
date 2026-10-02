"""place check points that sit on the body's surface (stickers, numbers,
badges) but show in only one photo: a ray from the photo's solved camera
through the reading's pixel, where it first meets the body, is the point
(reference.md, "How the model is matched to the photos", step 6). Adds
them to the body folder's triangulated.json, keeping what is there.

    blender -b --python scripts/mustang-gt/surface.py -- <body folder> <name> [<name> ...]
    blender -b --python scripts/mustang-gt/surface.py -- <body folder> all

`all` places every part in one_photo.txt: run it after every bundle.py,
which rewrites triangulated.json with only the parts two photos place.
"""

import json
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

HERE = Path(__file__).resolve().parent
args = sys.argv[sys.argv.index("--") + 1 :]
body, names = Path(args[0]), args[1:]
if names == ["all"]:
    names = [n for n in (HERE / "one_photo.txt").read_text().splitlines() if n.strip()]
cameras = json.loads((body / "cameras.json").read_text())
checks = json.loads((HERE / "checkpoints.json").read_text())
tri_path = body / "triangulated.json"
tri = json.loads(tri_path.read_text())
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(body / "prepared.glb"))
depsgraph = bpy.context.evaluated_depsgraph_get()
for name in names:
    # "name@other" places the point where its ray crosses the upright plane
    # (across the car) through the already placed point "other": for a
    # part standing clear of the body, in line with a part that touches it.
    name, _, through = name.partition("@")
    photo = next(p for p, pts in checks.items() if name in pts and p in cameras)
    cam = cameras[photo]
    u, v = checks[photo][name]
    width, height = cam["size"]
    rot = Matrix(cam["rot"])
    # The pixel's direction in the camera, then in the body's frame.
    ray = Vector(((u - width / 2) / cam["focal"], (v - height / 2) / cam["focal"], 1.0))
    direction = (rot.transposed() @ ray).normalized()
    if through.startswith("mirror "):
        # On the upright plane across the car at the mirror of a placed
        # point's distance out (a part the same on both sides).
        plane_y = -tri[through.removeprefix("mirror ")][1]
        start = Vector(cam["place"])
        where = start + direction * ((plane_y - start.y) / direction.y)
        tri[name] = [round(c, 4) for c in where]
        print(f"{name:28s} from {photo}, mirrored across from {through}: {tri[name]}")
        continue
    if through.startswith("level "):
        # A flat, level part at a measured height (the splitter plate).
        height = tri[through.removeprefix("level ")][2]
        start = Vector(cam["place"])
        where = start + direction * ((height - start.z) / direction.z)
        tri[name] = [round(c, 4) for c in where]
        print(f"{name:28s} from {photo}, level with {through}: {tri[name]}")
        continue
    if through.startswith("across "):
        # On the upright plane across the car at a placed point's place
        # along it: for a part on the splitter's front edge seen from the
        # front, whose rays cross that plane nearly square.
        plane_x = tri[through.removeprefix("across ")][0]
        start = Vector(cam["place"])
        where = start + direction * ((plane_x - start.x) / direction.x)
        tri[name] = [round(c, 4) for c in where]
        print(f"{name:28s} from {photo}, across the car at {through}: {tri[name]}")
        continue
    if through:
        plane_y = tri[through][1]
        start = Vector(cam["place"])
        where = start + direction * ((plane_y - start.y) / direction.y)
        tri[name] = [round(c, 4) for c in where]
        print(f"{name:28s} from {photo}, in line with {through}: {tri[name]}")
        continue
    hit, where, *_ = bpy.context.scene.ray_cast(depsgraph, Vector(cam["place"]), direction)
    if not hit:
        print(f"MISS {name}: the ray from {photo} meets nothing of the body")
        continue
    tri[name] = [round(c, 4) for c in where]
    print(f"{name:28s} from {photo}: {tri[name]}")
tri_path.write_text(json.dumps(tri, indent=1))
