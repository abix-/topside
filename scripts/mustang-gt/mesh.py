"""save the built model's triangles (assets/models/mustang.glb) in the
body's frame the cameras were solved in, for fitting cameras to the car's
outline (outline.py).

    blender -b --python scripts/mustang-gt/mesh.py -- <shift x> <lift> <out.npy> [<share>]

The shift and lift are as for overlay.py: the build's "moved by", the
shift being MUSTANG_LENGTH / 2 + its x. With a share (0.05), the model is
first reduced to that share of its triangles (Blender's Decimate), for
searching every view round the car quickly.
"""

import sys
from pathlib import Path

import bpy
import numpy as np

HERE = Path(__file__).resolve().parent
args = sys.argv[sys.argv.index("--") + 1 :]
shift, lift, out = args[:3]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(HERE.parent.parent / "assets" / "models" / "mustang.glb"))
if len(args) > 3:
    for o in bpy.context.scene.objects:
        if o.type == "MESH":
            o.modifiers.new("reduce", "DECIMATE").ratio = float(args[3])
depsgraph = bpy.context.evaluated_depsgraph_get()
triangles = []
for o in bpy.context.scene.objects:
    if o.type != "MESH":
        continue
    mesh = o.evaluated_get(depsgraph).to_mesh()
    mesh.calc_loop_triangles()
    world = o.matrix_world
    co = np.array([world @ v.co for v in mesh.vertices])
    idx = np.array([t.vertices[:] for t in mesh.loop_triangles])
    if len(idx):
        triangles.append(co[idx])
    o.to_mesh_clear()
tri = np.concatenate(triangles)
tri[:, :, 0] -= float(shift)
tri[:, :, 2] -= float(lift)
np.save(out, tri.astype(np.float32))
print("triangles", len(tri), "->", out)
