"""draw the built model straight down from above, on grey, to
scripts/mustang-gt/overlays/top.png."""

import bpy
from mathutils import Vector

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=r"C:\code\topside\assets\models\mustang.glb")
scene = bpy.context.scene
cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
scene.collection.objects.link(cam)
scene.camera = cam
cam.data.type = "ORTHO"
cam.data.ortho_scale = 5.4
cam.location = (0.0, 0.0, 20.0)
cam.rotation_euler = (0.0, 0.0, 0.0)
world = bpy.data.worlds.new("grey")
world.color = (0.35, 0.37, 0.40)
scene.world = world
scene.render.engine = "BLENDER_WORKBENCH"
scene.display.shading.color_type = "MATERIAL"
scene.display.shading.background_type = "WORLD"
scene.view_settings.view_transform = "Standard"
scene.render.resolution_x, scene.render.resolution_y = 1600, 1000
scene.render.filepath = r"C:\code\topside\scripts\mustang-gt\overlays\top.png"
bpy.ops.render.render(write_still=True)
