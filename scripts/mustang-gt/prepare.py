"""put a downloaded Mustang in the frame the reference points use: metres,
nose toward +x, the car's left toward +y, up +z, wheels on the ground at
z = 0, centred along and across; scaled to Ford's length (4.79 m). Writes
it as one .glb next to the source, and a picture from the front left to
check which way it faces.

    blender -b --python scripts/mustang-gt/prepare.py -- <source.glb> <out.glb> <turn degrees> <check.png>
"""

import math
import sys

import bpy
from mathutils import Matrix, Vector

source, out, turn, check = sys.argv[sys.argv.index("--") + 1 :][:4]
LENGTH = 4.79
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=source)
meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
bpy.ops.object.select_all(action="DESELECT")
for m in meshes:
    m.select_set(True)
bpy.context.view_layer.objects.active = meshes[0]
bpy.ops.object.make_single_user(type="SELECTED_OBJECTS", object=True, obdata=True)
bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
bpy.ops.object.join()
car = bpy.context.active_object
bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
car.data.transform(Matrix.Rotation(math.radians(float(turn)), 4, "Z"))
xs = [v.co.x for v in car.data.vertices]
ys = [v.co.y for v in car.data.vertices]
zs = [v.co.z for v in car.data.vertices]
print(f"as found: {max(xs) - min(xs):.3f} long, {max(ys) - min(ys):.3f} wide, {max(zs) - min(zs):.3f} tall")
scale = LENGTH / (max(xs) - min(xs))
car.data.transform(Matrix.Scale(scale, 4))
car.data.transform(Matrix.Translation((-(min(xs) + max(xs)) / 2 * scale, -(min(ys) + max(ys)) / 2 * scale, -min(zs) * scale)))
ys = [v.co.y for v in car.data.vertices]
zs = [v.co.z for v in car.data.vertices]
print(f"scaled x{scale:.4f}: {LENGTH:.3f} long, {max(ys) - min(ys):.3f} wide, {max(zs):.3f} tall")
bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", use_selection=False)
scene = bpy.context.scene
cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
scene.collection.objects.link(cam)
scene.camera = cam
cam.location = (7.0, 5.0, 2.5)
cam.rotation_euler = (Vector((0, 0, 0.7)) - cam.location).to_track_quat("-Z", "Y").to_euler()
scene.render.engine = "BLENDER_WORKBENCH"
scene.display.shading.color_type = "MATERIAL"
scene.render.resolution_x, scene.render.resolution_y = 1200, 700
scene.render.filepath = check
bpy.ops.render.render(write_still=True)
