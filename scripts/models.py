"""every model in the game, built in blender and rendered to a picture.

The Factorio way (docs/references.md "Graphics, how Factorio does it"):
each model is rendered ahead of time from the game's one camera, an
orthographic camera 45 degrees down looking north, lit by one sun from the
left at 45 degrees, with the cel shade baked in (design.md: a base colour
cut into hard light bands). The game draws only the pictures.

Units are metres, Blender is Z up. Every model rests on the ground (its
lowest point at z = 0) and its ground point (x = 0, y = 0) is the centre
of its picture.

Usage:
    blender -b --python scripts/models.py              # every model
    blender -b --python scripts/models.py -- pipe      # just these
"""

import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "pictures"

# Factorio's high resolution: 64 pixels to a tile, and a tile is a metre.
# The game draws the pictures at this scale (src/terrain.rs
# PICTURE_PIXELS_PER_METRE); change both together.
PIXELS_PER_METRE = 64
# The camera: 45 degrees down (FFF 269), looking north (+Y).
CAMERA_DOWN = math.radians(45)
# The sun: from the left at 45 degrees, so shadows fall to the right.
SUN_DOWN = math.radians(45)
# Space left around the model in the picture, in metres.
MARGIN = 0.1
# The cel shade: (light above this, brightness), hard steps, no blending.
BANDS = [(0.0, 0.45), (0.25, 0.75), (0.6, 1.0)]


# -- the scene ---------------------------------------------------------------


def clear():
    """empty the scene and set the renderer, colours and light."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    try:
        scene.render.engine = "BLENDER_EEVEE"
    except TypeError as e:
        sys.exit(f"ERROR: no eevee renderer: {e}")
    # AgX, Blender's default, shifts colours; Standard keeps them as set.
    scene.view_settings.view_transform = "Standard"
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", type="SUN"))
    sun.data.energy = 3.0
    # No shadows (design.md): the light still bands each face, but nothing
    # casts a shadow, not even one part of a model onto another.
    sun.data.use_shadow = False
    sun.rotation_euler = (0.0, -SUN_DOWN, 0.0)
    scene.collection.objects.link(sun)


def linear(c):
    """one sRGB channel (the game's Color::srgb numbers) as linear light."""
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def material(name, rgb, cutout=None):
    """the cel shade: diffuse light cut into BANDS, times the colour. With a
    `cutout` picture, its see-through parts stay see-through (leaf cards);
    none of its colour is used."""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    t = mat.node_tree
    t.nodes.clear()
    out = t.nodes.new("ShaderNodeOutputMaterial")
    diffuse = t.nodes.new("ShaderNodeBsdfDiffuse")
    to_rgb = t.nodes.new("ShaderNodeShaderToRGB")
    ramp = t.nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.interpolation = "CONSTANT"
    steps = ramp.color_ramp.elements
    while len(steps) < len(BANDS):
        steps.new(0.5)
    for step, (at, v) in zip(steps, BANDS):
        step.position = at
        step.color = (v, v, v, 1.0)
    mix = t.nodes.new("ShaderNodeMix")
    mix.data_type = "RGBA"
    mix.blend_type = "MULTIPLY"
    mix.inputs[0].default_value = 1.0  # factor
    mix.inputs[7].default_value = (*(linear(c) for c in rgb), 1.0)  # B colour
    emit = t.nodes.new("ShaderNodeEmission")
    t.links.new(diffuse.outputs[0], to_rgb.inputs[0])
    t.links.new(to_rgb.outputs[0], ramp.inputs[0])
    t.links.new(ramp.outputs[0], mix.inputs[6])  # A colour
    t.links.new(mix.outputs[2], emit.inputs[0])  # colour result
    if cutout is None:
        t.links.new(emit.outputs[0], out.inputs[0])
        return mat
    # Hard edged: a pixel is either leaf or nothing, like the cel bands.
    picture = t.nodes.new("ShaderNodeTexImage")
    picture.image = cutout
    solid = t.nodes.new("ShaderNodeMath")
    solid.operation = "GREATER_THAN"
    solid.inputs[1].default_value = 0.5
    clear_through = t.nodes.new("ShaderNodeBsdfTransparent")
    either = t.nodes.new("ShaderNodeMixShader")
    t.links.new(picture.outputs["Alpha"], solid.inputs[0])
    t.links.new(solid.outputs[0], either.inputs[0])  # factor
    t.links.new(clear_through.outputs[0], either.inputs[1])
    t.links.new(emit.outputs[0], either.inputs[2])
    t.links.new(either.outputs[0], out.inputs[0])
    return mat


# -- shapes ------------------------------------------------------------------


def _named(name, mat):
    obj = bpy.context.active_object
    obj.name = name
    if mat:
        obj.data.materials.append(mat)
    return obj


def cylinder(name, radius, length, sides=8, at=(0, 0, 0), along="Z", mat=None):
    """a closed cylinder of `length` along one axis, centred at `at`."""
    rotation = {"X": (0, math.pi / 2, 0), "Y": (math.pi / 2, 0, 0), "Z": (0, 0, 0)}[along]
    bpy.ops.mesh.primitive_cylinder_add(vertices=sides, radius=radius, depth=length, location=at, rotation=rotation)
    return _named(name, mat)


def box(name, size, at=(0, 0, 0), rotation=(0, 0, 0), mat=None):
    """an axis-aligned box of `size` (x, y, z), centred at `at`."""
    bpy.ops.mesh.primitive_cube_add(size=1.0, location=at, rotation=rotation)
    obj = _named(name, mat)
    obj.scale = size
    bpy.ops.object.transform_apply(scale=True)
    return obj


def load(file, colours):
    """import a model file (glTF) and put the cel shade on it. `colours`
    maps the start of each of its material names to (rgb, cut): with cut,
    the material keeps its picture's see-through parts (leaf cards) and
    nothing else of the picture."""
    bpy.ops.import_scene.gltf(filepath=str(file))
    meshes = [o for o in bpy.context.selected_objects if o.type == "MESH"]
    made = {}
    for obj in meshes:
        for slot in obj.material_slots:
            old = slot.material
            key = next((k for k in colours if old.name.startswith(k)), None)
            if key is None:
                sys.exit(f"ERROR: {file.name}: no colour for material {old.name}")
            if old.name not in made:
                rgb, cut = colours[key]
                image = next((n.image for n in old.node_tree.nodes if n.type == "TEX_IMAGE" and n.image), None)
                if cut and image is None:
                    sys.exit(f"ERROR: {file.name}: material {old.name} has no picture to cut from")
                made[old.name] = material(key, rgb, cutout=image if cut else None)
            slot.material = made[old.name]
    return meshes


def join(parts, name):
    """join parts into one flat-shaded object named `name`, resting on the ground."""
    bpy.ops.object.select_all(action="DESELECT")
    for p in parts:
        p.select_set(True)
    bpy.context.view_layer.objects.active = parts[0]
    if len(parts) > 1:
        bpy.ops.object.join()
    obj = bpy.context.active_object
    obj.name = name
    bpy.ops.object.shade_flat()
    obj.location.z -= min((obj.matrix_world @ v.co).z for v in obj.data.vertices)
    return obj


# -- the picture -------------------------------------------------------------


def picture(obj, out):
    """render `obj` from the game's camera to `out`, its ground point at the centre."""
    scene = bpy.context.scene
    cam = bpy.data.objects.new("camera", bpy.data.cameras.new("camera"))
    cam.data.type = "ORTHO"
    scene.collection.objects.link(cam)
    scene.camera = cam
    # Back the camera away from the ground point along its view; with an
    # orthographic camera the distance changes nothing but the clipping.
    back = Vector((0.0, -math.sin(CAMERA_DOWN), math.cos(CAMERA_DOWN)))
    cam.location = back * 100.0
    cam.rotation_euler = (CAMERA_DOWN, 0.0, 0.0)
    cam.data.clip_end = 300.0
    bpy.context.view_layer.update()
    # The picture's half width and half height: the furthest the model
    # reaches from the ground point, as the camera sees it.
    view = cam.matrix_world.inverted()
    seen = [view @ (obj.matrix_world @ v.co) for v in obj.data.vertices]
    half_w = max(abs(p.x) for p in seen) + MARGIN
    half_h = max(abs(p.y) for p in seen) + MARGIN
    w = math.ceil(2 * half_w * PIXELS_PER_METRE)
    h = math.ceil(2 * half_h * PIXELS_PER_METRE)
    scene.render.resolution_x = w
    scene.render.resolution_y = h
    scene.render.resolution_percentage = 100
    cam.data.sensor_fit = "HORIZONTAL"
    cam.data.ortho_scale = w / PIXELS_PER_METRE
    out.parent.mkdir(parents=True, exist_ok=True)
    scene.render.filepath = str(out)
    bpy.ops.render.render(write_still=True)
    print(f"{obj.name}: {len(obj.data.polygons)} faces, {w}x{h} pixels -> {out}")


# -- the models --------------------------------------------------------------


# Quaternius's Stylized Nature MegaKit, the free Standard version (CC0,
# scripts/megakit/License_Standard.txt), from
# https://quaternius.itch.io/stylized-nature-megakit. Only the files the
# models below use are kept; their bark pictures are left out, since the
# cel shade replaces them.
MEGAKIT = Path(__file__).resolve().parent / "megakit"


def tree():
    """a broadleaf tree: Quaternius's CommonTree_1, in grey-brown bark and
    green leaves; the leaf cards keep their leaf shapes."""
    colours = {"Bark": ((0.36, 0.31, 0.26), False), "Leaves": ((0.18, 0.32, 0.12), True)}
    return join(load(MEGAKIT / "CommonTree_1.gltf", colours), "tree")


def pipe():
    """the pipe: a metre of rusty steel with a taped grip at one end, lying on the ground."""
    length, radius = 1.0, 0.025
    steel = material("steel", (0.45, 0.42, 0.40))
    tape = material("tape", (0.18, 0.16, 0.14))
    parts = [
        cylinder("shaft", radius, length, along="X", mat=steel),
        # The grip: a fatter band over the near end.
        cylinder("grip", radius * 1.25, 0.22, along="X", at=(-length / 2 + 0.14, 0, 0), mat=tape),
        # A threaded collar at the far end, so the hitting end reads.
        cylinder("collar", radius * 1.4, 0.05, along="X", at=(length / 2 - 0.04, 0, 0), mat=steel),
        # A dent: a shallow box pressed into the shaft a third of the way up.
        box("dent", (0.06, radius * 1.1, radius * 0.5), at=(0.1, 0, radius * 0.9), mat=steel),
    ]
    return join(parts, "pipe")


MODELS = {"tree": tree, "pipe": pipe}


def main():
    args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    names = args or list(MODELS)
    unknown = [n for n in names if n not in MODELS]
    if unknown:
        sys.exit(f"ERROR: no model named {', '.join(unknown)}; models: {', '.join(MODELS)}")
    for name in names:
        clear()
        picture(MODELS[name](), OUT / f"{name}.png")


main()
