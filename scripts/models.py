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
import random
import sys
from pathlib import Path

import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "pictures"

# Factorio's high resolution: 64 pixels to a tile, and a tile is a metre.
# The game draws the pictures at this scale (src/picture.rs
# PIXELS_PER_METRE); change both together.
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
    """the cel shade: diffuse light cut into BANDS, times the colour. `rgb`
    is the game's sRGB numbers, or a picture the model takes each face's
    flat colour from (a Kenney model's palette). With a `cutout` picture,
    its see-through parts stay see-through (leaf cards); none of its
    colour is used."""
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
    if isinstance(rgb, bpy.types.Image):
        palette = t.nodes.new("ShaderNodeTexImage")
        palette.image = rgb
        palette.interpolation = "Closest"  # swatches stay flat
        t.links.new(palette.outputs["Color"], mix.inputs[7])  # B colour
    else:
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
    maps the start of each of its material names to (rgb, cut): rgb None
    keeps the model's own colours from its picture (a palette), a Path
    takes them from that picture file instead; with cut, the material
    keeps its picture's see-through parts (leaf cards) and nothing else of
    the picture."""
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
                if isinstance(rgb, Path):
                    rgb = bpy.data.images.load(str(rgb), check_existing=True)
                if (cut or rgb is None) and image is None:
                    sys.exit(f"ERROR: {file.name}: material {old.name} has no picture to take from")
                made[old.name] = material(key, image if rgb is None else rgb, cutout=image if cut else None)
            slot.material = made[old.name]
    return meshes


def join(parts, name, size=None, turn=0):
    """join parts into one flat-shaded object named `name`, turned `turn`
    degrees about upright, resting on the ground. With `size`, it is
    scaled evenly so its largest side is that many metres (the game's size
    for it)."""
    bpy.ops.object.select_all(action="DESELECT")
    for p in parts:
        p.select_set(True)
    bpy.context.view_layer.objects.active = parts[0]
    if len(parts) > 1:
        bpy.ops.object.join()
    obj = bpy.context.active_object
    obj.name = name
    bpy.ops.object.shade_flat()
    # The glTF importer leaves objects turned by quaternion.
    obj.rotation_mode = "XYZ"
    obj.rotation_euler.z += math.radians(turn)
    bpy.context.view_layer.update()
    if size is not None:
        obj.scale *= size / max(obj.dimensions)
        bpy.context.view_layer.update()
    # Centred on its footprint, its lowest point on the ground: the ground
    # point is the middle of what it stands on.
    world = [obj.matrix_world @ v.co for v in obj.data.vertices]
    obj.location.x -= (min(p.x for p in world) + max(p.x for p in world)) / 2
    obj.location.y -= (min(p.y for p in world) + max(p.y for p in world)) / 2
    obj.location.z -= min(p.z for p in world)
    bpy.context.view_layer.update()
    return obj


# -- the picture -------------------------------------------------------------


def picture(obj, out, frame=None, fit=None):
    """render `obj` (one object or several) from the game's camera to `out`,
    its ground point at the centre. The picture fits the model, or the
    objects in `fit` (so pictures of pieces drawn over each other share one
    frame), or with `frame`, is that many metres either side of the ground
    point (half width, half height) as the camera sees it, so pictures that
    are drawn over each other line up."""
    objs = obj if isinstance(obj, list) else [obj]
    fit = fit or objs
    scene = bpy.context.scene
    # Square tiles (FFF 269: the camera is 45 degrees down, "but in Factorio
    # ... our tiles being square"): the scene is stretched along the ground
    # away from the camera by 1/cos(45), so a metre of ground is a metre
    # on screen both ways, while heights still show as the camera sees them.
    stretch = bpy.data.objects.get("square tiles")
    if stretch is None:
        stretch = bpy.data.objects.new("square tiles", None)
        stretch.scale = (1.0, 1.0 / math.cos(CAMERA_DOWN), 1.0)
        scene.collection.objects.link(stretch)
    for o in scene.objects:
        if o.parent is None and o is not stretch and o.type in {"MESH", "ARMATURE"}:
            o.parent = stretch
    bpy.context.view_layer.update()
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
    if frame is None:
        view = cam.matrix_world.inverted()
        seen = [view @ (o.matrix_world @ v.co) for o in fit for v in o.data.vertices]
        half_w = max(abs(p.x) for p in seen) + MARGIN
        half_h = max(abs(p.y) for p in seen) + MARGIN
    else:
        half_w, half_h = frame
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
    bpy.data.objects.remove(cam, do_unlink=True)
    faces = sum(len(o.data.polygons) for o in objs)
    print(f"{objs[0].name}: {faces} faces, {w}x{h} pixels -> {out}")


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


def rock():
    """the forest's rock: Quaternius's Rock_Medium_1, 0.8 m across (the
    forest rock in content/src/lib.rs), in its grey."""
    colours = {"Rocks": ((0.4, 0.4, 0.35), False)}
    return join(load(MEGAKIT / "Rock_Medium_1.gltf", colours), "rock", size=0.8)


# -- items ---------------------------------------------------------------------
#
# Every item lies on the ground at the game's size for it (src/interact.rs
# ITEM_SIZE and DEPLOYABLE_SIZE; change both together), its picture named
# after it (content/src/lib.rs). Items come from Kenney's CC0 kits in their
# own flat colours, or are built here from shapes in the colours content
# gives them.
ITEM = 0.35
DEPLOYABLE = 0.7


def kenney(kit, model, name, size=ITEM, rgb=None, turn=0):
    """a model from one of Kenney's kits (scripts/kenney-<kit>, CC0), in
    its own flat colours, or all in `rgb`; turned `turn` degrees about
    upright, so it lies the way content sizes it."""
    return join(load(ROOT / "scripts" / f"kenney-{kit}" / f"{model}.glb", {"colormap": (rgb, False)}), name, size, turn)


# -- weapons, in layers ----------------------------------------------------------
#
# A melee weapon is a handle and a head (design.md "What 2D changes"). Every
# handle is built lying along x, its grip at x = 0 and its tip at x = its
# length, its middle at the height of its radius, so it rests on the ground;
# every head is built on that tip. Handles differ in length, so each head is
# rendered once per handle, as each hair is once per body: the handle alone
# is `<handle>` (the item's own picture) and each head alone on it is
# `<handle> <head>`, all of one handle's pictures in one frame, so they stack.
# Every weapon is drawn at WEAPON_SCALE, so a metre-long pipe lies ITEM long
# and a head is the same size on every handle.
WEAPON_SCALE = ITEM


def pipe_handle():
    """the pipe: a metre of rusty steel with a taped grip at one end."""
    length, radius = 1.0, 0.025
    steel = material("steel", (0.45, 0.42, 0.40))
    tape = material("tape", (0.18, 0.16, 0.14))
    parts = [
        cylinder("shaft", radius, length, along="X", at=(length / 2, 0, radius), mat=steel),
        # The grip: a fatter band over the near end.
        cylinder("grip", radius * 1.25, 0.22, along="X", at=(0.14, 0, radius), mat=tape),
        # A threaded collar at the far end, so the hitting end reads.
        cylinder("collar", radius * 1.4, 0.05, along="X", at=(length - 0.04, 0, radius), mat=steel),
        # A dent: a shallow box pressed into the shaft past the middle.
        box("dent", (0.06, radius * 1.1, radius * 0.5), at=(0.6, 0, radius * 1.9), mat=steel),
    ]
    return parts, length, radius


def wooden_handle():
    """the wooden handle: a short length of grey-brown wood, its grip
    wrapped in cloth."""
    length, radius = 0.7, 0.022
    wood = material("wood", (0.4, 0.28, 0.16))
    cloth = material("wrap", (0.5, 0.46, 0.38))
    parts = [
        cylinder("shaft", radius, length, along="X", at=(length / 2, 0, radius), mat=wood),
        cylinder("grip", radius * 1.2, 0.18, along="X", at=(0.1, 0, radius), mat=cloth),
    ]
    return parts, length, radius


def long_pole():
    """the long pole: nearly two metres of pale wood, wrapped where both
    hands hold it."""
    length, radius = 1.8, 0.025
    wood = material("pole", (0.48, 0.37, 0.22))
    cloth = material("wrap", (0.5, 0.46, 0.38))
    parts = [
        cylinder("shaft", radius, length, along="X", at=(length / 2, 0, radius), mat=wood),
        cylinder("grip_near", radius * 1.2, 0.16, along="X", at=(0.2, 0, radius), mat=cloth),
        cylinder("grip_far", radius * 1.2, 0.16, along="X", at=(0.8, 0, radius), mat=cloth),
    ]
    return parts, length, radius


def nails(tip, radius):
    """nails driven through the last hand's width of the handle, sticking
    out to both sides and up."""
    steel = material("nail", (0.55, 0.55, 0.58))
    parts = []
    # Drawn thicker and longer than real nails, so they read at item size.
    for i in range(6):
        x = tip - 0.03 - i * 0.04
        if i % 3 == 2:
            parts.append(cylinder(f"nail_{i}", 0.009, 0.14, sides=6, along="Z", at=(x, 0, radius + 0.06), mat=steel))
        else:
            side = 1 if i % 3 == 0 else -1
            parts.append(cylinder(f"nail_{i}", 0.009, 0.14, sides=6, along="Y", at=(x, side * 0.06, radius), mat=steel))
    return parts


def scrap_blade(tip, radius):
    """a strip of rusty scrap sharpened along one edge, bound to the tip."""
    rust = material("scrap", (0.5, 0.35, 0.25))
    edge = material("edge", (0.65, 0.63, 0.6))
    tape = material("tape", (0.18, 0.16, 0.14))
    return [
        box("blade", (0.28, 0.07, 0.006), at=(tip + 0.1, 0, radius), mat=rust),
        box("edge", (0.26, 0.012, 0.007), at=(tip + 0.1, 0.035, radius), mat=edge),
        cylinder("binding", radius * 1.35, 0.07, along="X", at=(tip - 0.03, 0, radius), mat=tape),
    ]


def axe_head(tip, radius):
    """an axe head across the tip: the blade to one side, its bright edge
    ground, a short poll to the other."""
    steel = material("axe", (0.32, 0.32, 0.34))
    edge = material("edge", (0.65, 0.63, 0.6))
    return [
        box("cheek", (0.08, 0.16, 0.03), at=(tip - 0.05, 0.06, radius), mat=steel),
        box("edge", (0.1, 0.02, 0.031), at=(tip - 0.05, 0.145, radius), mat=edge),
        box("poll", (0.06, 0.05, 0.04), at=(tip - 0.05, -0.04, radius), mat=steel),
    ]


def spear_point(tip, radius):
    """a steel point past the tip, bound on with wire."""
    steel = material("point", (0.5, 0.5, 0.52))
    wire = material("wire", (0.35, 0.3, 0.25))
    bpy.ops.mesh.primitive_cone_add(vertices=4, radius1=0.055, radius2=0, depth=0.3, location=(tip + 0.15, 0, radius), rotation=(0, math.pi / 2, 0))
    return [
        _named("point", steel),
        cylinder("binding", radius * 1.3, 0.08, along="X", at=(tip - 0.02, 0, radius), mat=wire),
    ]


def sharpened_road_sign(tip, radius):
    """a piece of green road sign with its white border, cut to a point
    and bolted on past the tip."""
    sign = material("sign", (0.2, 0.5, 0.2))
    white = material("border", (0.9, 0.9, 0.88))
    bolt = material("bolt", (0.55, 0.55, 0.58))
    return [
        box("plate", (0.24, 0.13, 0.006), at=(tip + 0.08, 0, radius), mat=sign),
        box("point", (0.1, 0.1, 0.006), at=(tip + 0.2, 0, radius), rotation=(0, 0, math.pi / 4), mat=sign),
        box("border", (0.24, 0.015, 0.007), at=(tip + 0.08, 0.058, radius), mat=white),
        cylinder("bolt", 0.012, 0.02, along="Z", at=(tip - 0.02, 0, radius + 0.01), mat=bolt),
    ]


# The handles and heads, named as in content/src/lib.rs (register_items,
# register_layers).
HANDLES = {"pipe": pipe_handle, "wooden handle": wooden_handle, "long pole": long_pole}
HEADS = {
    "nails": nails,
    "scrap blade": scrap_blade,
    "axe head": axe_head,
    "spear point": spear_point,
    "sharpened road sign": sharpened_road_sign,
}


def merge(parts, name):
    """join parts into one flat-shaded object named `name`, left where it
    was built, its origin at the world's."""
    bpy.ops.object.select_all(action="DESELECT")
    for p in parts:
        p.select_set(True)
    bpy.context.view_layer.objects.active = parts[0]
    if len(parts) > 1:
        bpy.ops.object.join()
    obj = bpy.context.active_object
    obj.name = name
    bpy.ops.object.shade_flat()
    bpy.context.scene.cursor.location = (0.0, 0.0, 0.0)
    bpy.ops.object.origin_set(type="ORIGIN_CURSOR")
    return obj


def weapons():
    """render every handle alone and every head alone on each handle, one
    frame per handle, the handle's middle at the ground point."""
    for handle, build in HANDLES.items():
        clear()
        parts, length, radius = build()
        layers = {handle: merge(parts, handle)}
        for head, put_on in HEADS.items():
            layers[f"{handle} {head}"] = merge(put_on(length, radius), head)
        for obj in layers.values():
            obj.scale = (WEAPON_SCALE,) * 3
            obj.location = (-length / 2 * WEAPON_SCALE, 0.0, 0.0)
        bpy.context.view_layer.update()
        every = list(layers.values())
        for name, obj in layers.items():
            for other in every:
                other.hide_render = other is not obj
            picture(obj, OUT / f"{name}.png", fit=every)


def pipe_shotgun():
    """the pipe shotgun: a steel pipe barrel slid into a fatter sleeve, on a
    rough wooden stock."""
    steel = material("steel", (0.3, 0.3, 0.3))
    wood = material("wood", (0.4, 0.28, 0.16))
    parts = [
        cylinder("barrel", 0.022, 0.6, along="X", at=(0.15, 0, 0), mat=steel),
        cylinder("sleeve", 0.032, 0.16, along="X", at=(-0.2, 0, 0), mat=steel),
        box("stock", (0.3, 0.045, 0.09), at=(-0.42, 0, -0.02), mat=wood),
        box("grip", (0.05, 0.04, 0.11), at=(-0.25, 0, -0.07), mat=wood),
    ]
    return join(parts, "pipe shotgun", ITEM)


def shotgun_shell():
    """three shotgun shells side by side: red hulls on brass bases."""
    hull = material("hull", (0.8, 0.3, 0.2))
    brass = material("brass", (0.75, 0.6, 0.25))
    parts = []
    for i in range(3):
        y = (i - 1) * 0.03
        parts.append(cylinder(f"hull_{i}", 0.012, 0.055, along="X", at=(0, y, 0), mat=hull))
        parts.append(cylinder(f"base_{i}", 0.0125, 0.014, along="X", at=(-0.034, y, 0), mat=brass))
    return join(parts, "shotgun shell", ITEM)


def note():
    """a note: a sheet of paper with lines of writing."""
    paper = material("paper", (0.85, 0.82, 0.7))
    ink = material("ink", (0.2, 0.2, 0.25))
    parts = [box("sheet", (0.2, 0.26, 0.004), mat=paper)]
    for i in range(5):
        parts.append(box(f"line_{i}", (0.14 - (i % 2) * 0.03, 0.008, 0.001), at=(0, 0.08 - i * 0.04, 0.0025), mat=ink))
    return join(parts, "note", ITEM)


def bandage():
    """a bandage: a white roll with its loose end lying out."""
    cloth = material("gauze", (0.9, 0.88, 0.84))
    parts = [
        cylinder("roll", 0.035, 0.07, along="Y", mat=cloth),
        box("end", (0.12, 0.06, 0.004), at=(0.09, 0, -0.033), mat=cloth),
    ]
    return join(parts, "bandage", ITEM)


def road_sign_vest():
    """the road sign vest: a green road sign with a white border, worn on
    two straps."""
    sign = material("sign", (0.2, 0.5, 0.2))
    white = material("border", (0.9, 0.9, 0.88))
    strap = material("strap", (0.18, 0.16, 0.14))
    w, h = 0.42, 0.5
    parts = [
        box("plate", (w, h, 0.03), mat=sign),
        box("top", (w, 0.02, 0.002), at=(0, h / 2 - 0.02, 0.016), mat=white),
        box("bottom", (w, 0.02, 0.002), at=(0, -h / 2 + 0.02, 0.016), mat=white),
        box("left", (0.02, h, 0.002), at=(-w / 2 + 0.02, 0, 0.016), mat=white),
        box("right", (0.02, h, 0.002), at=(w / 2 - 0.02, 0, 0.016), mat=white),
        box("strap_l", (0.07, 0.16, 0.02), at=(-0.12, h / 2 + 0.06, 0), mat=strap),
        box("strap_r", (0.07, 0.16, 0.02), at=(0.12, h / 2 + 0.06, 0), mat=strap),
    ]
    return join(parts, "road sign vest", ITEM)


# -- the minor sites' props ----------------------------------------------------
#
# Each at its largest side as content/src/lib.rs sizes it (the minor
# sites' `prop`); change both together.


def berry_bush():
    """a berry bush: Quaternius's flowering bush, its flowers the berries,
    1.0 m across (content/src/lib.rs)."""
    colours = {"Leaves": ((0.18, 0.32, 0.12), True), "Flowers": ((0.55, 0.15, 0.3), True)}
    return join(load(MEGAKIT / "Bush_Common_Flowers.gltf", colours), "berry bush", 1.0)


def well():
    """a well: a ring of stones round dark water, two posts and a crossbar,
    Kenney's bucket hanging from it, 1.4 m across (content/src/lib.rs)."""
    stone = material("stone", (0.45, 0.47, 0.5))
    water = material("water", (0.12, 0.18, 0.28))
    wood = material("wood", (0.4, 0.28, 0.16))
    parts = [cylinder("water", 0.5, 0.05, sides=12, at=(0, 0, 0.5), mat=water)]
    for i in range(12):
        a = i * math.tau / 12
        parts.append(box(f"stone_{i}", (0.3, 0.16, 0.55), at=(0.58 * math.cos(a), 0.58 * math.sin(a), 0.275), rotation=(0, 0, a + math.pi / 2), mat=stone))
    for x in (-0.6, 0.6):
        parts.append(box("post", (0.08, 0.08, 1.3), at=(x, 0, 0.65), mat=wood))
    parts.append(box("bar", (1.3, 0.07, 0.07), at=(0, 0, 1.25), mat=wood))
    bucket = load(ROOT / "scripts" / "kenney-survival" / "bucket.glb", {"colormap": (None, False)})
    for b in bucket:
        b.location = (0, 0, 0.8)
    return join(parts + bucket, "well", 1.4)


def pellet():
    """a shotgun pellet in flight (src/combat.rs), drawn 0.18 m across so it
    can be seen."""
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=0.09)
    return join([_named("pellet", material("brass", (1.0, 0.85, 0.3)))], "pellet")


def parachute():
    """a supply drop's parachute lying flat: a canvas disc with its seams."""
    canvas = material("canvas", (0.6, 0.6, 0.62))
    seam = material("seam", (0.45, 0.45, 0.47))
    parts = [cylinder("canopy", 1.5, 0.05, sides=16, mat=canvas)]
    for i in range(8):
        a = i * math.pi / 8
        parts.append(box(f"seam_{i}", (3.0, 0.03, 0.01), at=(0, 0, 0.03), rotation=(0, 0, a), mat=seam))
    return join(parts, "parachute", 3.0)


# -- the ground ------------------------------------------------------------------
#
# Every kind of ground gets GROUND_VARIANTS tile pictures (as Factorio's
# tiles have variations), near white with light and dark detail, which the
# game tints with the kind's colour (content/src/lib.rs ground_color). They
# are stacked kind by kind, variant by variant, into one strip,
# assets/ground.png, the game's ground tileset (outside assets/pictures,
# which the game loads as plain pictures): the kinds in this
# order (src/terrain.rs GROUND_KINDS; change both together).
GROUND_KINDS = ["grass", "dirt", "sand", "forest floor", "rubble", "road", "water", "cliff", "out of map"]
GROUND_VARIANTS = 4
TILE_PIXELS = PIXELS_PER_METRE  # a tile is a metre


def ground_tile(kind, rng):
    """one tile of `kind`: a near-white tile with its detail, kept off the
    tile's edges so neighbouring tiles join."""
    base = material("ground", (0.85, 0.85, 0.85))
    light = material("light", (1.0, 1.0, 1.0))
    dark = material("dark", (0.65, 0.65, 0.65))
    shade = lambda: light if rng.random() < 0.5 else dark
    spot = lambda edge=0.38: (rng.uniform(-edge, edge), rng.uniform(-edge, edge))
    parts = [box("tile", (1.0, 1.0, 0.01), at=(0, 0, -0.005), mat=base)]

    def flat(name, r, mat, sides=6):
        x, y = spot()
        parts.append(cylinder(name, r, 0.004, sides=sides, at=(x, y, 0.002), mat=mat))

    def stroke(name, length, width, mat, height=0.004):
        x, y = spot(0.3)
        parts.append(box(name, (length, width, height), at=(x, y, height / 2), rotation=(0, 0, rng.uniform(0, math.pi)), mat=mat))

    if kind == "grass":
        for i in range(12):
            x, y = spot()
            m = shade()
            for j in range(3):
                h = rng.uniform(0.05, 0.09)
                bpy.ops.mesh.primitive_cone_add(vertices=4, radius1=0.012, radius2=0, depth=h, location=(x + (j - 1) * 0.012, y, h / 2), rotation=(rng.uniform(-0.3, 0.3), rng.uniform(-0.3, 0.3), 0))
                parts.append(_named(f"blade_{i}_{j}", m))
    elif kind == "forest floor":
        for i in range(10):
            flat(f"leaf_{i}", rng.uniform(0.03, 0.05), shade(), sides=5)
        for i in range(2):
            stroke(f"twig_{i}", rng.uniform(0.12, 0.2), 0.012, dark, 0.01)
    elif kind == "dirt":
        for i in range(3):
            flat(f"patch_{i}", rng.uniform(0.08, 0.14), dark, sides=7)
        for i in range(8):
            bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=rng.uniform(0.015, 0.035), location=(*spot(), 0.01))
            parts.append(_named(f"pebble_{i}", shade()))
    elif kind == "sand":
        for i in range(5):
            stroke(f"ripple_{i}", rng.uniform(0.3, 0.5), 0.015, shade())
    elif kind == "rubble":
        for i in range(10):
            s = rng.uniform(0.04, 0.12)
            x, y = spot()
            parts.append(box(f"chunk_{i}", (s, s * rng.uniform(0.5, 1.0), s * 0.5), at=(x, y, s * 0.25), rotation=(0, 0, rng.uniform(0, math.pi)), mat=shade()))
    elif kind == "road":
        for i in range(3):
            stroke(f"crack_{i}", rng.uniform(0.15, 0.35), 0.008, dark, 0.002)
        flat("patch", rng.uniform(0.06, 0.1), dark)
    elif kind == "water":
        for i in range(4):
            stroke(f"ripple_{i}", rng.uniform(0.2, 0.35), 0.01, light, 0.002)
    elif kind == "cliff":
        for i in range(3):
            x, y = spot(0.25)
            bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=rng.uniform(0.15, 0.22), location=(x, y, 0))
            rock = _named(f"rock_{i}", shade())
            rock.scale.z = 0.35
            parts.append(rock)
    return parts


def ground():
    """render every ground tile and stack them into assets/ground.png."""
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        files = []
        for k, kind in enumerate(GROUND_KINDS):
            for v in range(GROUND_VARIANTS):
                clear()
                parts = ground_tile(kind, random.Random(k * 100 + v))
                files.append(Path(tmp) / f"{kind} {v}.png")
                picture(parts, files[-1], frame=(0.5, 0.5))
        # Blender keeps an image's rows bottom first, and the game reads the
        # strip's first tile at the top: stack the last tile at the bottom.
        pixels = []
        for f in reversed(files):
            tile = bpy.data.images.load(str(f))
            if tuple(tile.size) != (TILE_PIXELS, TILE_PIXELS):
                sys.exit(f"ERROR: ground tile {f.name} is {tuple(tile.size)}, not {TILE_PIXELS} square")
            pixels.extend(tile.pixels[:])
        strip = bpy.data.images.new("ground", TILE_PIXELS, TILE_PIXELS * len(files), alpha=True)
        strip.pixels[:] = pixels
        out = OUT.parent / "ground.png"
        strip.filepath_raw = str(out)
        strip.file_format = "PNG"
        strip.save()
        print(f"ground: {len(files)} tiles, {TILE_PIXELS}x{TILE_PIXELS * len(files)} pixels -> {out}")


# -- buildings -----------------------------------------------------------------
#
# A building is drawn a tile at a time, as Factorio draws its walls
# (src/structure.rs): every piece is one tile (a metre) across, near white,
# and the game tints it the building's colour. Walls are drawn 1 m tall, so
# a wall seen from the camera does not hide the room behind it.
WALL_HEIGHT = 1.0
LIGHT = (0.92, 0.92, 0.92)


def wall():
    """one tile of wall: a block WALL_HEIGHT tall."""
    return join([box("block", (1.0, 1.0, WALL_HEIGHT), mat=material("wall", LIGHT))], "wall")


def furniture():
    """one tile of furniture (shelves, a counter): a block a little smaller
    than its tile."""
    return join([box("block", (0.9, 0.9, 0.7), mat=material("furniture", LIGHT))], "furniture")


DOOR = (0.35, 0.25, 0.15)  # modforge structure.rs DOOR_COLOR


def door():
    """one tile of a closed door: a wooden panel across the tile, as tall
    as a wall, with its planks and a handle."""
    wood = material("door", DOOR)
    dark = material("plank", (0.25, 0.18, 0.1))
    metal = material("handle", (0.6, 0.6, 0.6))
    parts = [box("panel", (1.0, 0.12, WALL_HEIGHT), at=(0, 0, WALL_HEIGHT / 2), mat=wood)]
    for i in range(1, 4):
        parts.append(box(f"plank_{i}", (0.015, 0.125, WALL_HEIGHT * 0.95), at=(-0.5 + i * 0.25, 0, WALL_HEIGHT / 2), mat=dark))
    parts.append(box("handle", (0.06, 0.16, 0.04), at=(0.35, 0, WALL_HEIGHT * 0.55), mat=metal))
    return join(parts, "door")


def door_open():
    """one tile of an open door: the same panel swung a quarter turn, seen
    edge on along its hinge side."""
    return join([door()], "door open", turn=90)


def floor():
    """one tile of floor, with darker seams along its edges so the tiles
    show."""
    tile = material("floor", LIGHT)
    seam = material("seam", (0.75, 0.75, 0.75))
    parts = [box("tile", (1.0, 1.0, 0.02), mat=tile)]
    for i, (w, h, x, y) in enumerate([(1.0, 0.03, 0, 0.485), (1.0, 0.03, 0, -0.485), (0.03, 1.0, 0.485, 0), (0.03, 1.0, -0.485, 0)]):
        parts.append(box(f"seam_{i}", (w, h, 0.004), at=(x, y, 0.011), mat=seam))
    return join(parts, "floor")


# -- people ------------------------------------------------------------------
#
# People are built from pieces (design.md "What 2D changes"): each is layers
# drawn per facing, every layer rendered on one rig and pose with the other
# layers held out, so what one piece hides of another is already cut away.
# The rig is Quaternius's Universal Base Characters (CC0,
# scripts/quaternius-characters/License_Standard.txt), from
# https://quaternius.itch.io/universal-base-characters.
CHARACTERS = Path(__file__).resolve().parent / "quaternius-characters"
# Facings, as the game picks them (src/actor.rs): 0 east, then a turn of
# 45 degrees counter-clockwise each, so 2 is north, away from the camera.
FACINGS = 8
# Every person picture is this many metres either side of the ground point
# (half width, half height), so all layers and facings line up.
PERSON_FRAME = (0.6, 1.45)
# The bodies, each with its light skin (the game tints it darker) and the
# pieces that fit its head: hairstyles, and the beard.
BODIES = {
    "male": ("Superhero_Male_FullBody.gltf", "T_Superhero_Male_Ligh.png", ["SimpleParted", "Buzzed", "Long", "Beard"]),
    "female": ("Superhero_Female_FullBody.gltf", "T_Superhero_Female_Light_BaseColor.png", ["Buns", "BuzzedFemale", "Long"]),
}


# The rig's animations: Quaternius's Universal Animation Library (CC0,
# scripts/quaternius-animations/License.txt), from
# https://quaternius.itch.io/universal-animation-library, made for this rig.
ANIMATIONS = Path(__file__).resolve().parent / "quaternius-animations" / "UAL1_Standard.glb"
# The poses a person is rendered in: standing (one frame of the idle), and
# walking (WALK_FRAMES frames spread over one loop of the walk).
STAND = ("Idle_Loop", 0)
WALK = "Walk_Loop"
WALK_FRAMES = 8


def animations():
    """bring in the animation library's actions, and nothing of its own
    mannequin."""
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=str(ANIMATIONS))
    for o in set(bpy.data.objects) - before:
        bpy.data.objects.remove(o, do_unlink=True)


def pose(rig, action, frame):
    """put the rig in one frame of one of the library's actions."""
    act = bpy.data.actions[action]
    rig.animation_data_create()
    rig.animation_data.action = act
    if act.slots:
        rig.animation_data.action_slot = act.slots[0]
    bpy.context.scene.frame_set(frame)
    bpy.context.view_layer.update()


def person(file, skin):
    """a body: its rig and meshes (with eyes and brows), in its light skin."""
    meshes = load(CHARACTERS / file, {"MI_Superhero": (CHARACTERS / skin, False), "MI_": (None, False)})
    # The file carries a loose helper ball; the body is what the rig holds.
    for m in [m for m in meshes if m.parent is None]:
        bpy.data.objects.remove(m, do_unlink=True)
    body = [m for m in meshes if m.parent is not None]
    return body[0].parent, body


def piece(name, rig):
    """one hairstyle or the beard, pinned to the rig's head."""
    meshes = load(CHARACTERS / f"Hair_{name}.gltf", {"MI_": (None, False)})
    for h in meshes:
        world = h.matrix_world.copy()
        h.parent, h.parent_type, h.parent_bone = rig, "BONE", "Head"
        h.matrix_world = world
    return meshes


def vest(rig, body):
    """the road sign vest worn: a green road sign with a white border on the
    chest, another on the back, joined by straps over the shoulders; sized
    from this body's chest and pinned to its chest bone, standing at rest."""
    sign = material("sign", (0.2, 0.5, 0.2))
    white = material("border", (0.9, 0.9, 0.88))
    strap = material("strap", (0.18, 0.16, 0.14))
    # The torso below the armpits: at rest the arms stand straight out at
    # shoulder height, so only what is near the middle is chest.
    world = (o.matrix_world @ v.co for o in body for v in o.data.vertices)
    chest = [p for p in world if 1.05 < p.z < 1.3 and abs(p.x) < 0.3]
    front, back = min(p.y for p in chest), max(p.y for p in chest)
    half = (max(p.x for p in chest) - min(p.x for p in chest)) / 2 * 0.8
    w, h, z = half * 2, 0.4, 1.25
    parts = []
    for side, y in (("front", front - 0.02), ("back", back + 0.02)):
        parts.append(box(f"{side}_plate", (w, 0.02, h), at=(0, y, z), mat=sign))
        out = -0.011 if side == "front" else 0.011
        for i, (bw, bh, bx, bz) in enumerate([(w, 0.02, 0, z + h / 2 - 0.02), (w, 0.02, 0, z - h / 2 + 0.02), (0.02, h, -w / 2 + 0.02, z), (0.02, h, w / 2 - 0.02, z)]):
            parts.append(box(f"{side}_border_{i}", (bw, 0.002, bh), at=(bx, y + out, bz), mat=white))
    for x in (-half * 0.55, half * 0.55):
        parts.append(box("strap", (0.06, back - front + 0.06, 0.03), at=(x, (front + back) / 2, z + h / 2 + 0.05), mat=strap))
    for p in parts:
        world = p.matrix_world.copy()
        p.parent, p.parent_type, p.parent_bone = rig, "BONE", "spine_03"
        p.matrix_world = world
    return parts


# What people wear, drawn as a layer over the body like a piece: by the
# item's picture name (content/src/lib.rs), built on each body.
WORN = {"road sign vest": vest}


def people():
    """render every body and every piece that fits it, per facing, standing
    and in each walking frame: `person <body> <layer> <facing>` and
    `person <body> <layer> <facing> walk <frame>`, the layer being `body`
    or a piece. A piece is rendered with the body held out, so whatever of
    it is behind the body is cut away and any piece draws over the body;
    the body is rendered whole, so every piece fits it."""
    for body_name, (file, skin, names) in BODIES.items():
        clear()
        animations()
        rig, body = person(file, skin)
        rig.rotation_mode = "XYZ"
        pieces = {n.lower(): piece(n, rig) for n in names}
        worn = {item: build(rig, body) for item, build in WORN.items()}
        layers = {"body": body, **pieces, **worn}
        pose(rig, *STAND)
        # Which way the rig faces standing: from its ankles to its toes.
        bones, m = rig.pose.bones, rig.matrix_world
        toes = (m @ bones["ball_l"].head + m @ bones["ball_r"].head) / 2
        ankles = (m @ bones["foot_l"].head + m @ bones["foot_r"].head) / 2
        rest = math.atan2(toes.y - ankles.y, toes.x - ankles.x)

        def render(suffix):
            for name, meshes in layers.items():
                held_out = body if name != "body" else []
                for other in layers.values():
                    for o in other:
                        o.hide_render = o not in meshes and o not in held_out
                        o.is_holdout = o in held_out
                picture(meshes, OUT / f"person {body_name} {name} {suffix}.png", frame=PERSON_FRAME)

        length = int(bpy.data.actions[WALK].frame_range[1])
        for k in range(FACINGS):
            rig.rotation_euler = (0, 0, math.radians(k * 360 / FACINGS) - rest)
            pose(rig, *STAND)
            render(f"{k}")
            for f in range(WALK_FRAMES):
                pose(rig, WALK, f * length // WALK_FRAMES)
                render(f"{k} walk {f}")


# Models that render several pictures themselves.
SETS = {"person": people, "ground": ground, "weapons": weapons}


MODELS = {
    "tree": tree,
    "rock": rock,
    # The wasteland's and the ruins' scatter, at their largest side as
    # content/src/lib.rs sizes them.
    "fallen log": lambda: kenney("survival", "tree-log", "fallen log", 1.2, rgb=(0.5, 0.42, 0.32), turn=90),
    "broken wall": lambda: kenney("graveyard", "stone-wall-damaged", "broken wall", 2.0, rgb=(0.45, 0.43, 0.4)),
    # What people eat and drink from where it stands.
    "berry bush": berry_bush,
    "well": well,
    # What flies.
    "pellet": pellet,
    # Items, named as in content/src/lib.rs.
    "note": note,
    "canned food": lambda: kenney("food", "can", "canned food"),
    "water bottle": lambda: kenney("survival", "bottle", "water bottle"),
    "bandage": bandage,
    "storage box": lambda: kenney("survival", "box", "storage box", DEPLOYABLE),
    "scrap": lambda: kenney("survival", "metal-panel-screws-half", "scrap"),
    "cloth": lambda: kenney("survival", "bedroll-packed", "cloth"),
    "pipe shotgun": pipe_shotgun,
    "shotgun shell": shotgun_shell,
    "road sign vest": road_sign_vest,
    # Buildings, a tile at a time (src/structure.rs).
    "wall": wall,
    "furniture": furniture,
    "floor": floor,
    "door": door,
    "door open": door_open,
    # The minor sites' props, named as in content/src/lib.rs.
    # A wrecked car: Kenney's sedan all in rust, lying along x (4.2 m long).
    "car wreck": lambda: kenney("car", "sedan", "car wreck", 4.2, rgb=(0.45, 0.2, 0.15), turn=90),
    "barrel": lambda: kenney("survival", "barrel", "barrel", 0.9),
    "tent": lambda: kenney("survival", "tent", "tent", 2.4),
    "fire pit": lambda: kenney("survival", "campfire-pit", "fire pit", 1.0),
    "crate": lambda: kenney("survival", "box", "crate", 0.5),
    "supply crate": lambda: kenney("survival", "box-large", "supply crate", 1.6),
    "parachute": parachute,
    "grave": lambda: kenney("graveyard", "grave", "grave", 1.8),
    "stone": lambda: kenney("graveyard", "gravestone-broken", "stone", 0.5),
}


def main():
    args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    names = args or list(MODELS) + list(SETS)
    unknown = [n for n in names if n not in MODELS and n not in SETS]
    if unknown:
        sys.exit(f"ERROR: no model named {', '.join(unknown)}; models: {', '.join(list(MODELS) + list(SETS))}")
    for name in names:
        if name in SETS:
            SETS[name]()
            continue
        clear()
        picture(MODELS[name](), OUT / f"{name}.png")


main()
