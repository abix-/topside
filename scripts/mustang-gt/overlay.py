"""draw the built model (assets/models/mustang.glb) from a photo's solved
camera over the photo itself, at half strength, so what does not line up
shows (reference.md, "How the model is matched to the photos", step 6).

    blender -b --python scripts/mustang-gt/overlay.py -- <photo> <body folder> <shift x> <out.png>

The shift is how far the built model's frame lies from the body's frame
the cameras were solved in: the build prints it ("moved by"), and the
body's frame is centred along the car, so the shift is
MUSTANG_LENGTH / 2 + that x.
"""

import json
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

HERE = Path(__file__).resolve().parent
args = sys.argv[sys.argv.index("--") + 1 :]
name, body, shift, out = args[:4]
# How far up the build moved the car (the build's "moved by" z), if at all.
lift = float(args[4]) if len(args) > 4 else 0.0
cam_data = json.loads((Path(body) / "cameras.json").read_text())[name]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(HERE.parent.parent / "assets" / "models" / "mustang.glb"))
for o in bpy.context.scene.objects:
    if o.parent is None:
        o.location.x -= float(shift)
        o.location.z -= lift
scene = bpy.context.scene
width, height = cam_data["size"]
cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
scene.collection.objects.link(cam)
scene.camera = cam
cam.data.sensor_fit = "HORIZONTAL"
cam.data.sensor_width = 36.0
cam.data.lens = cam_data["focal"] * 36.0 / width
cam.data.clip_start = 1.0
cam.data.clip_end = 10000.0
# OpenCV's camera looks along +z with y down; Blender's along -z with y up.
rot = Matrix(cam_data["rot"]).transposed() @ Matrix.Diagonal((1.0, -1.0, -1.0))
cam.matrix_world = Matrix.Translation(Vector(cam_data["place"])) @ rot.to_4x4()
scene.render.engine = "BLENDER_WORKBENCH"
scene.display.shading.light = "STUDIO"
scene.display.shading.color_type = "MATERIAL"
scene.view_settings.view_transform = "Standard"
scene.render.film_transparent = True
scene.render.resolution_x, scene.render.resolution_y = width, height
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = "PNG"
scene.render.image_settings.color_mode = "RGBA"
render = Path(out).with_suffix(".render.png")
scene.render.filepath = str(render)
bpy.ops.render.render(write_still=True)

# Laid over the photo at half strength.
import numpy as np

photo = bpy.data.images.load(str(next(Path("C:/car").glob(f"*{name}.jpg"))))
drawn = bpy.data.images.load(str(render))
p = np.array(photo.pixels[:]).reshape(height, width, 4)
d = np.array(drawn.pixels[:]).reshape(height, width, 4)
a = d[:, :, 3:4] * 0.5
p[:, :, :3] = p[:, :, :3] * (1 - a) + d[:, :, :3] * a
photo.pixels[:] = p.ravel()
photo.filepath_raw = out
photo.file_format = "PNG"
photo.save()
print("overlay", out)
