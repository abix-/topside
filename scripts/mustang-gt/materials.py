"""list a model's materials: each one's colour, whether it has a picture,
how many faces use it, and where those faces are (the box round them, in
the reference frame).

    blender -b --python scripts/mustang-gt/materials.py -- <model.glb>
"""

import sys

import bpy
from mathutils import Vector

source = sys.argv[sys.argv.index("--") + 1]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=source)
found = {}
for o in bpy.context.scene.objects:
    if o.type != "MESH":
        continue
    for p in o.data.polygons:
        m = o.material_slots[p.material_index].material if o.material_slots else None
        c = o.matrix_world @ p.center
        found.setdefault(m.name if m else "(none)", []).append(c)
for name, cs in sorted(found.items(), key=lambda kv: -len(kv[1])):
    m = bpy.data.materials.get(name)
    colour, picture = None, False
    if m and m.use_nodes:
        b = next((n for n in m.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
        if b:
            colour = tuple(round(v, 2) for v in b.inputs["Base Color"].default_value[:3])
        picture = any(n.type == "TEX_IMAGE" for n in m.node_tree.nodes)
    lo = [round(min(c[i] for c in cs), 2) for i in range(3)]
    hi = [round(max(c[i] for c in cs), 2) for i in range(3)]
    print(f"{name:40s} {len(cs):7d} faces  colour {colour}  picture {picture}  from {lo} to {hi}")
