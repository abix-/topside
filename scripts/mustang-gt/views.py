"""find the reference points on the model (reference.md, "How the model is
matched to the photos", step 3).

    blender -b --python scripts/mustang-gt/views.py -- render <dir>
        draws assets/models/mustang.glb square on from the side, front,
        rear and top (orthographic, on grey) into <dir>, and writes
        <dir>/views.json: how each picture's pixels map onto the model.
    blender -b --python scripts/mustang-gt/views.py -- cast <dir>
        reads scripts/mustang-gt/picks.json (for each point: the view and
        the pixel it was picked at in that view's picture) and traces each
        pixel into the model, writing its 3D position to
        scripts/mustang-gt/modelpoints.json.

Positions are in the model's own frame as Blender imports it: metres, the
nose toward +x, the car's left toward +y, up +z, the ground at z = 0.
"""

import json
import sys
from pathlib import Path

import bpy
from mathutils import Vector

HERE = Path(__file__).resolve().parent
args = sys.argv[sys.argv.index("--") + 1 :]
mode, out = args[:2]
out = Path(out)
# Another model to find the points on (a candidate body), and its own
# picks and points files; by default the game's model and the files here.
MODEL = Path(args[2]) if len(args) > 2 else HERE.parent.parent / "assets" / "models" / "mustang.glb"
PICKS = Path(args[3]) if len(args) > 3 else HERE / "picks.json"
POINTS = Path(args[4]) if len(args) > 4 else HERE / "modelpoints.json"

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(MODEL))
car = next(o for o in bpy.data.objects if o.type == "MESH")
corners = [car.matrix_world @ Vector(c) for c in car.bound_box]
middle = Vector([(min(c[i] for c in corners) + max(c[i] for c in corners)) / 2 for i in range(3)])

# Each view: where the camera looks from (relative to the car's middle),
# how many metres the picture is across, and its pixels.
VIEWS = {
    "left": ((0.0, 30.0, 0.0), 5.8, (2400, 1200)),
    "right": ((0.0, -30.0, 0.0), 5.8, (2400, 1200)),
    "front": ((30.0, 0.0, 0.0), 2.6, (1600, 1000)),
    "rear": ((-30.0, 0.0, 0.0), 2.6, (1600, 1000)),
    "top": ((0.0, 0.0, 30.0), 5.8, (2400, 1200)),
}


def camera_frame(offset):
    """the picture's right, up and forward directions for a camera at
    `offset` from the car's middle, looking at it."""
    from mathutils import Matrix

    forward = (-Vector(offset)).normalized()
    # Up in the picture is up in the world, or the nose when looking down.
    up = Vector((0.0, 0.0, 1.0)) if abs(forward.z) < 0.9 else Vector((1.0, 0.0, 0.0))
    right = forward.cross(up).normalized()
    up = right.cross(forward).normalized()
    turn = Matrix((right, up, -forward)).transposed()
    return right, up, forward, turn


if mode == "render":
    scene = bpy.context.scene
    world = bpy.data.worlds.new("grey")
    world.color = (0.35, 0.37, 0.40)
    scene.world = world
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "MATERIAL"
    scene.display.shading.show_cavity = False
    scene.display.shading.background_type = "WORLD"
    scene.view_settings.view_transform = "Standard"
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    scene.collection.objects.link(cam)
    scene.camera = cam
    cam.data.type = "ORTHO"
    cam.data.clip_start = 1.0
    cam.data.clip_end = 100.0
    out.mkdir(parents=True, exist_ok=True)
    maps = {}
    for name, (offset, across, (width, height)) in VIEWS.items():
        right, up, forward, turn = camera_frame(offset)
        cam.matrix_world = turn.to_4x4()
        cam.location = middle + Vector(offset)
        cam.data.ortho_scale = across
        scene.render.resolution_x, scene.render.resolution_y = width, height
        scene.render.filepath = str(out / f"view_{name}.png")
        bpy.ops.render.render(write_still=True)
        maps[name] = {
            "centre": list(middle + Vector(offset)),
            "right": list(right),
            "up": list(up),
            "forward": list(forward),
            "across": across,
            "size": [width, height],
        }
    (out / "views.json").write_text(json.dumps(maps, indent=1))
    print("rendered", ", ".join(VIEWS))

elif mode == "cast":
    maps = json.loads((out / "views.json").read_text())
    picks = json.loads(PICKS.read_text())
    found = {}
    depsgraph = bpy.context.evaluated_depsgraph_get()
    for point, (view, px, py) in picks.items():
        m = maps[view]
        width, height = m["size"]
        metre = m["across"] / width
        start = Vector(m["centre"]) + Vector(m["right"]) * (px - width / 2) * metre + Vector(m["up"]) * (height / 2 - py) * metre
        hit, where, *_ = bpy.context.scene.ray_cast(depsgraph, start, Vector(m["forward"]))
        if not hit:
            print(f"MISS {point}: nothing of the model at pixel {px}, {py} in {view}")
            continue
        found[point] = [round(c, 4) for c in where]
    POINTS.write_text(json.dumps(found, indent=1))
    print(f"{len(found)} of {len(picks)} points traced into the model")
