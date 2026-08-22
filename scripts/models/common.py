"""shapes for scripted low poly models, run inside blender (bpy).

A model script builds from these, joins into ONE object, and calls
export(). Units are metres. Blender is Z up; the glTF exporter turns
that into Bevy's Y up, so model with Z as up and X as the long axis.
"""

import sys
from pathlib import Path

import bpy


def clear():
    """empty the default scene."""
    bpy.ops.wm.read_factory_settings(use_empty=True)


def material(name, rgb):
    """flat colour material; the game swaps it for cel shading."""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*rgb, 1.0)
    bsdf.inputs["Roughness"].default_value = 1.0
    return mat


def cylinder(name, radius, length, sides=8, at=(0, 0, 0), along="X", mat=None):
    """a closed cylinder of `length` along one axis, centred at `at`."""
    rotation = {"X": (0, 1.5707963, 0), "Y": (1.5707963, 0, 0), "Z": (0, 0, 0)}[along]
    bpy.ops.mesh.primitive_cylinder_add(
        vertices=sides, radius=radius, depth=length, location=at, rotation=rotation
    )
    obj = bpy.context.active_object
    obj.name = name
    if mat:
        obj.data.materials.append(mat)
    return obj


def box(name, size, at=(0, 0, 0), rotation=(0, 0, 0), mat=None):
    """an axis-aligned box of `size` (x, y, z), centred at `at`."""
    bpy.ops.mesh.primitive_cube_add(size=1.0, location=at, rotation=rotation)
    obj = bpy.context.active_object
    obj.name = name
    obj.scale = size
    bpy.ops.object.transform_apply(scale=True)
    if mat:
        obj.data.materials.append(mat)
    return obj


def join(parts, name):
    """join parts into one flat-shaded object named `name`."""
    bpy.ops.object.select_all(action="DESELECT")
    for p in parts:
        p.select_set(True)
    bpy.context.view_layer.objects.active = parts[0]
    bpy.ops.object.join()
    obj = bpy.context.active_object
    obj.name = name
    bpy.ops.object.shade_flat()
    return obj


def export(obj, out):
    """export `obj` alone as a .glb at `out` and print the face count."""
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    Path(out).parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=str(out),
        export_format="GLB",
        use_selection=True,
        export_apply=True,
        export_yup=True,
    )
    print(f"{obj.name}: {len(obj.data.polygons)} faces, {len(obj.data.vertices)} vertices -> {out}")


def out_path():
    """the output path passed after `--` on blender's command line."""
    args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    if not args:
        sys.exit("usage: blender -b --python <model>.py -- <out.glb>")
    return Path(args[0])
