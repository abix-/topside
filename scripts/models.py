"""every model in the game, built in blender and exported for the game.

Each model is exported as glTF (assets/models/<name>.glb) and drawn live
by the game in 3D (authority.md "Presentation"), which adds the cel shade
(design.md: a base colour cut into hard light bands). A model here carries
only its colours: a flat colour per material, or a palette picture (a
Kenney model's), with see-through parts kept for leaf cards.

Units are metres, Blender is Z up (the exporter turns it Y up). Every model
rests on the ground (its lowest point at z = 0) with its ground point at
x = 0, y = 0.

Usage:
    blender -b --python scripts/models.py              # every model
    blender -b --python scripts/models.py -- pipe      # just these
"""

import json
import math
import sys
from pathlib import Path

import bpy

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "models"


# -- the scene ---------------------------------------------------------------


def clear():
    """empty the scene."""
    bpy.ops.wm.read_factory_settings(use_empty=True)


def linear(c):
    """one sRGB channel (the game's Color::srgb numbers) as linear light."""
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def material(name, rgb, cutout=None, clear=None):
    """a flat material the exporter carries to the game: `rgb` is the
    game's sRGB numbers, or a picture the model takes each face's flat
    colour from (a Kenney model's palette). With a `cutout` picture, its
    see-through parts stay see-through (leaf cards), hard edged; none of
    its colour is used. With `clear` (how solid it is, 0 to 1) it is
    tinted glass that can be seen through. No shine: the game's cel shade
    does the light."""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    t = mat.node_tree
    bsdf = next(n for n in t.nodes if n.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Roughness"].default_value = 1.0
    if isinstance(rgb, bpy.types.Image):
        palette = t.nodes.new("ShaderNodeTexImage")
        palette.image = rgb
        palette.interpolation = "Closest"  # swatches stay flat
        t.links.new(palette.outputs["Color"], bsdf.inputs["Base Color"])
    else:
        bsdf.inputs["Base Color"].default_value = (*(linear(c) for c in rgb), 1.0)
        # The icons' renderer (Workbench) reads this, not the nodes.
        mat.diffuse_color = (*(linear(c) for c in rgb), 1.0)
    if cutout is not None:
        leaf = t.nodes.new("ShaderNodeTexImage")
        leaf.image = cutout
        # Hard edged: a pixel is either leaf or nothing. The exporter reads
        # a Round on the alpha as glTF's alpha mask at one half.
        solid = t.nodes.new("ShaderNodeMath")
        solid.operation = "ROUND"
        t.links.new(leaf.outputs["Alpha"], solid.inputs[0])
        t.links.new(solid.outputs[0], bsdf.inputs["Alpha"])
    if clear is not None:
        # The exporter reads an Alpha below 1 as glTF's blended alpha.
        bsdf.inputs["Alpha"].default_value = clear
        mat.diffuse_color = (*mat.diffuse_color[:3], clear)
        mat.surface_render_method = "BLENDED"
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
                # Named as the model named it, so its parts can still be
                # found by material.
                made[old.name] = material(old.name, image if rgb is None else rgb, cutout=image if cut else None)
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


# -- the export ----------------------------------------------------------------


def export(obj, name):
    """export `obj` (one object or several, with what hangs off them) and
    nothing else as assets/models/<name>.glb, Y up as the game is."""
    objs = obj if isinstance(obj, list) else [obj]
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
        for child in o.children_recursive:
            child.select_set(True)
    out = OUT / f"{name}.glb"
    out.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=str(out), export_format="GLB", use_selection=True, export_yup=True)
    faces = sum(len(o.data.polygons) for o in objs if o.type == "MESH")
    print(f"{name}: {faces} faces -> {out}")


# -- the icons -------------------------------------------------------------------
#
# Every item's model is also rendered to a picture, its icon in an inventory
# slot (design.md: the inventory screen, item icons rendered from the
# models): seen from the front and above, on nothing.

ICONS = ROOT / "assets" / "icons"
ICON_PIXELS = 128
# Where the icon camera looks from: in front (-y) and above.
ICON_VIEW = (0.0, -1.0, 0.7)
# Room left round the item, as a share of its size.
ICON_ROOM = 1.15


def icon(frame, shown, name):
    """render `shown` (one object or several) alone as assets/icons/<name>.png,
    framed to fit every object of `frame`: the layers of one item share a
    frame, so their icons, drawn in one slot, stack as the layers do."""
    from mathutils import Vector

    frame = frame if isinstance(frame, list) else [frame]
    shown = shown if isinstance(shown, list) else [shown]
    shown = {c for o in shown for c in [o, *o.children_recursive]}
    scene = bpy.context.scene
    if scene.camera is None:
        cam = bpy.data.objects.new("icon camera", bpy.data.cameras.new("icon camera"))
        scene.collection.objects.link(cam)
        scene.camera = cam
    cam = scene.camera
    for o in scene.objects:
        o.hide_render = o not in shown and o is not cam
    view = Vector(ICON_VIEW).normalized()
    cam.rotation_euler = (-view).to_track_quat("-Z", "Y").to_euler()
    bpy.context.view_layer.update()
    right = cam.matrix_world.to_3x3() @ Vector((1, 0, 0))
    up = cam.matrix_world.to_3x3() @ Vector((0, 1, 0))
    corners = [o.matrix_world @ Vector(c) for f in frame for o in [f, *f.children_recursive] for c in o.bound_box]
    across = [c.dot(right) for c in corners]
    tall = [c.dot(up) for c in corners]
    middle = right * (min(across) + max(across)) / 2 + up * (min(tall) + max(tall)) / 2
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = max(max(across) - min(across), max(tall) - min(tall)) * ICON_ROOM
    cam.data.clip_start = 0.01
    cam.data.clip_end = 100.0
    cam.location = middle + view * 20.0
    # Flat colours as the models carry them, lit enough to show their shape.
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "TEXTURE"
    scene.view_settings.view_transform = "Standard"
    scene.render.film_transparent = True
    scene.render.resolution_x = scene.render.resolution_y = ICON_PIXELS
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    out = ICONS / f"{name}.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    scene.render.filepath = str(out)
    bpy.ops.render.render(write_still=True)
    print(f"{name}: icon -> {out}")


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
# exported once per handle, as each hair is once per body: the handle alone
# is `<handle>` (the item's own model) and each head alone on it is
# `<handle> <head>`, all of one handle's models in one frame, so they stack.
# Weapons are their real size, and a head the same size on every handle.
WEAPON_SCALE = 1.0


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


def layered(base, parts, layers, middle):
    """export `base` alone as `<base>`, and each of its layers alone on it as
    `<base> <layer>`, all in one frame at WEAPON_SCALE, `middle` (metres
    along x) at the ground point and the lowest point of any of them on the
    ground, so drawn at one place they stack. `layers` maps each layer's
    name to its parts."""
    objs = {base: merge(parts, base)}
    for name, layer_parts in layers.items():
        objs[f"{base} {name}"] = merge(layer_parts, name)
    every = list(objs.values())
    low = min(v.co.z for o in every for v in o.data.vertices)
    for obj in every:
        obj.scale = (WEAPON_SCALE,) * 3
        obj.location = (-middle * WEAPON_SCALE, 0.0, -low * WEAPON_SCALE)
    bpy.context.view_layer.update()
    for name, obj in objs.items():
        export(obj, name)
    for name, obj in objs.items():
        icon(every, obj, name)


# -- the pipe shotgun, in layers -----------------------------------------------
#
# A gun is a body, barrel, grip, stock, and sight (design.md "What 2D
# changes"). The body is built lying along x, the muzzle toward +x, its middle
# at x = 0; every part is built where it goes on the body.


def pipe_shotgun_body():
    """the pipe shotgun's body: the fat steel sleeve the barrel slides into,
    and a bent strip of steel for a trigger guard."""
    steel = material("steel", (0.3, 0.3, 0.3))
    return [
        cylinder("sleeve", 0.032, 0.16, along="X", mat=steel),
        box("guard", (0.05, 0.012, 0.035), at=(-0.03, 0, -0.045), mat=steel),
    ]


def barrel(length):
    """a steel pipe barrel of `length`, slid into the sleeve."""
    steel = material("barrel", (0.34, 0.34, 0.35))
    return [
        cylinder("barrel", 0.022, length, along="X", at=(-0.05 + length / 2, 0, 0), mat=steel),
        cylinder("muzzle", 0.026, 0.03, along="X", at=(-0.05 + length - 0.015, 0, 0), mat=steel),
    ]


def wooden_grip():
    """a rough wooden grip under the back of the sleeve."""
    wood = material("wood", (0.4, 0.28, 0.16))
    return [box("grip", (0.05, 0.04, 0.12), at=(-0.07, 0, -0.08), rotation=(0, -0.25, 0), mat=wood)]


def taped_grip():
    """a grip of scrap wrapped in black tape."""
    tape = material("tape", (0.14, 0.13, 0.12))
    band = material("band", (0.3, 0.28, 0.25))
    return [
        box("grip", (0.05, 0.042, 0.12), at=(-0.07, 0, -0.08), rotation=(0, -0.25, 0), mat=tape),
        box("band", (0.055, 0.046, 0.015), at=(-0.075, 0, -0.1), rotation=(0, -0.25, 0), mat=band),
    ]


def wooden_stock():
    """a rough plank of a stock behind the sleeve."""
    wood = material("wood", (0.4, 0.28, 0.16))
    return [box("stock", (0.32, 0.045, 0.09), at=(-0.24, 0, -0.03), mat=wood)]


def nail_sight():
    """a nail standing up on the front of the sleeve, to aim along."""
    steel = material("nail", (0.55, 0.55, 0.58))
    return [cylinder("sight", 0.006, 0.05, sides=6, along="Z", at=(0.06, 0, 0.05), mat=steel)]


# The pipe shotgun's parts, named as in content/src/lib.rs. No stock and no
# sight are no layer at all.
GUN_PARTS = {
    "pipe barrel": lambda: barrel(0.6),
    "short pipe barrel": lambda: barrel(0.35),
    "wooden grip": wooden_grip,
    "taped grip": taped_grip,
    "wooden stock": wooden_stock,
    "nail sight": nail_sight,
}


def weapons():
    """render every handle with its heads, and the pipe shotgun with its
    parts, each in its own frame."""
    for handle, build in HANDLES.items():
        clear()
        parts, length, radius = build()
        # The grip at the ground point, so a hand holds it there.
        layered(handle, parts, {head: put_on(length, radius) for head, put_on in HEADS.items()}, 0.0)
    clear()
    layered("pipe shotgun", pipe_shotgun_body(), {name: build() for name, build in GUN_PARTS.items()}, 0.0)


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


def folded(name, rgb):
    """clothes folded flat on the ground: a cloth square with its fold."""
    cloth = material(name, rgb)
    fold = material(f"{name} fold", tuple(c * 0.7 for c in rgb))
    parts = [
        box("cloth", (0.3, 0.25, 0.04), mat=cloth),
        box("fold", (0.3, 0.01, 0.002), at=(0, 0, 0.021), mat=fold),
    ]
    return join(parts, name, ITEM)


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


def tap():
    """The Tap's tap (content/episodes/the-tap.yaml): a rusty pipe up out of
    the floor, a valve wheel on it, a spout bent over a drain with water
    running. Its look is Claude's (2026-11-19), for the operator to change.
    1.1 m tall."""
    rust = material("rust", (0.42, 0.24, 0.14))
    iron = material("iron", (0.3, 0.3, 0.32))
    water = material("water", (0.3, 0.45, 0.7))
    parts = [
        cylinder("drain", 0.28, 0.06, sides=12, at=(0.25, 0, 0.03), mat=iron),
        cylinder("pipe", 0.05, 1.0, at=(0, 0, 0.5), mat=rust),
        cylinder("spout", 0.045, 0.3, along="X", at=(0.13, 0, 0.95), mat=rust),
        cylinder("lip", 0.045, 0.12, at=(0.27, 0, 0.9), mat=rust),
        cylinder("valve", 0.12, 0.03, along="Y", at=(0, -0.09, 0.7), mat=iron),
        cylinder("stem", 0.02, 0.1, along="Y", at=(0, -0.05, 0.7), mat=iron),
        cylinder("running", 0.02, 0.8, at=(0.27, 0, 0.44), mat=water),
    ]
    return join(parts, "tap", 1.1)


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


def gantry():
    """the launch site's gantry, the big thing on the horizon (design.md
    "The first hour"): a steel lattice tower 60 m tall, four legs braced
    every six metres, a red light-band at its top."""
    steel = material("steel", (0.55, 0.55, 0.58))
    red = material("band", (0.7, 0.2, 0.15))
    w, h, step = 6.0, 60.0, 6.0
    parts = []
    for x in (-w / 2, w / 2):
        for y in (-w / 2, w / 2):
            parts.append(box("leg", (0.5, 0.5, h), at=(x, y, h / 2), mat=steel))
    for i in range(1, int(h / step) + 1):
        z = i * step
        for side, (sx, sy) in enumerate([(w, 0.3), (w, 0.3), (0.3, w), (0.3, w)]):
            x = 0 if side < 2 else (-w / 2 if side == 2 else w / 2)
            y = (-w / 2 if side == 0 else w / 2) if side < 2 else 0
            parts.append(box("brace", (sx, sy, 0.3), at=(x, y, z), mat=steel))
    parts.append(box("band", (w + 0.6, w + 0.6, 2.0), at=(0, 0, h - 1.0), mat=red))
    return join(parts, "gantry")


# -- the mustang ---------------------------------------------------------------
#
# The operator's blue Mustang GT, #776, as in their twelve track photos
# (C:\car). The car itself is supercarmodels' "2018 Ford Mustang GT (S550)"
# (CC BY, https://sketchfab.com/3d-models/2018-ford-mustang-gt-s550-123bf7e107d64530a5b862aba4afb8b2,
# scripts/mustang-gt/candidate-b), chosen because it measured closest to
# the photos (scripts/mustang-gt/reference.md, "Which downloaded body"),
# in this car's colours. What the operator's car has on top (the wing on
# its Texas end plates, the splitter, the hood vents, the numbers, the tow
# hook) is built here and laid onto the car's own surface. It lies along
# x, its nose toward +x, its rear bumper at x = 0.
MUSTANG_GT = Path(__file__).resolve().parent / "mustang-gt" / "candidate-b" / "prepared.glb"
MUSTANG_LENGTH = 4.79
# The one black every black part of the car is: trim, plastic, tyres,
# roof, trunk, hood, mirrors, wing, splitter, wheels, numbers.
MUSTANG_BLACK = (0.035, 0.035, 0.04)
# The model's material names for its paint, glass and tyres.
BODY = "Ford_MustangGTFastbackTNR0_2018"
PAINT = BODY + "PaintTNR"
GLASS = BODY + "Window"
# The tyres: the outer ring of the model's wheels, made black in
# mustang_car().
TYRE = "tyre"
# Its wheels (this car's black ones are built in mustang()).
MODEL_WHEELS = ("Ford_MustangMach1_2021_Wheel1A", "RIM_B")
# The model's colours by its material names, in this car's colours; None
# keeps the model's own picture.
MUSTANG_COLOURS = {
    PAINT: ((0.01, 0.30, 0.70), False),
    BODY + "Coloured": (MUSTANG_BLACK, False),  # black trim
    BODY + "Base": (MUSTANG_BLACK, False),  # black plastic
    GLASS: ((0.08, 0.10, 0.12), False),
    # The model's own rims (every face of it lies inside a wheel): taken
    # off in mustang_car(), the car's measured ones built in their place.
    "material": (MUSTANG_BLACK, False),
    "Ford_MustangMach1_2021_Callipers": (MUSTANG_BLACK, False),
    "RED_GLASS": ((0.80, 0.06, 0.06), False),  # the taillights
    # The cabin seen through the open windows: the median colour of the
    # cabin through the door window in 1073, sRGB (1, 32, 44).
    # The grilles: the median colour of the upper grille in 2428, sRGB
    # (8, 33, 55).
    BODY + "Grille": ((0.031, 0.129, 0.216), False),
    # The pony in the grille (the only badge kept) is blacked out on this
    # car: the median of three patches on it in 2428 and 1164, sRGB
    # (33, 40, 57).
    BODY + "BadgeA": ((0.129, 0.157, 0.224), False),
    BODY + "Interior": ((0.004, 0.125, 0.173), False),
    BODY + "SeatBelt": ((0.004, 0.125, 0.173), False),
    BODY + "Textured": ((0.004, 0.125, 0.173), False),
    "Ford_Mustang": (None, False),  # lights, badges, grilles, wheels, inside: their own pictures
    "RIM_B": (None, False),
}


def mustang_car():
    """supercarmodels' Mustang in this car's colours as one object, lying
    along x with its rear bumper at x = 0, without what this car does not
    have: its wheels, its rear spoiler and its rear GT badge."""
    import bmesh
    from mathutils import Matrix

    meshes = load(MUSTANG_GT, MUSTANG_COLOURS)
    bpy.ops.object.select_all(action="DESELECT")
    for m in meshes:
        m.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    if len(meshes) > 1:
        bpy.ops.object.join()
    car = bpy.context.active_object
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    # prepared.glb is centred along the car: its rear bumper to x = 0.
    rear = min(v.co.x for v in car.data.vertices)
    car.data.transform(Matrix.Translation((-rear, 0.0, 0.0)))
    bm = bmesh.new()
    bm.from_mesh(car.data)
    wheels = {i for i, s in enumerate(car.material_slots) if s.material.name.startswith(MODEL_WHEELS)}
    badges = {i for i, s in enumerate(car.material_slots) if "Badge" in s.material.name}
    # Its wheels and tyres are one material: the tyre, the outer ring of
    # each, is kept and made black; the rim inside it goes.
    car.data.materials.append(material("tyre", MUSTANG_BLACK))
    tyre_slot = len(car.material_slots) - 1
    gone = []
    for front in (False, True):
        for side in (-1, 1):
            wheel = [f for f in bm.faces if f.material_index in wheels and (f.calc_center_median().x > MUSTANG_LENGTH / 2) == front and f.calc_center_median().y * side > 0]
            if not wheel:
                continue
            cs = [f.calc_center_median() for f in wheel]
            mid_x = (min(c.x for c in cs) + max(c.x for c in cs)) / 2
            mid_z = (min(c.z for c in cs) + max(c.z for c in cs)) / 2
            outer = max(math.hypot(c.x - mid_x, c.z - mid_z) for c in cs)
            # The tyre moves to where the car's hub truly is (measured from the
            # photos, reference.md "Ride height"), up and down and along the
            # car: the car sits lower on its wheels than the model.
            where = "front" if front else "rear"
            name = f"{where} hub {'left' if side > 0 else 'right'}"
            dx = measured(f"body {name}")[0] - (MODEL_POINTS[name][0] + MUSTANG_LENGTH / 2)
            dz = measured(f"body {name}")[2] - MODEL_POINTS[name][2]
            # And its size: the car's tyre as measured (TYRE_DIAMETER), the
            # model's being its hub's height (it stands on the ground).
            k = TYRE_DIAMETER[where] / 2 / MODEL_POINTS[name][2]
            moved = set()
            for f, c in zip(wheel, cs):
                if math.hypot(c.x - mid_x, c.z - mid_z) > outer * RIM_OF_TYRE:
                    f.material_index = tyre_slot
                    for v in f.verts:
                        if v not in moved:
                            v.co.x = mid_x + dx + (v.co.x - mid_x) * k
                            v.co.z = mid_z + dz + (v.co.z - mid_z) * k
                            moved.add(v)
                else:
                    gone.append(f)
    # Its badges, all but the pony in the grille: this car's own 5.0 and
    # 776 are built in mustang(), and it has no GT badge on the rear.
    gone += [f for f in bm.faces if f.material_index in badges and f.calc_center_median().x < MUSTANG_LENGTH - 0.3]
    # The model's own rims, a material of their own ("material"): the
    # car's ten spoke rims are built in mustang() (rim()).
    rims = {i for i, s in enumerate(car.material_slots) if s.material.name.split(".")[0] == "material"}
    gone += [f for f in bm.faces if f.material_index in rims]
    # The GT letters raised on the black panel between the taillights, part
    # of the panel's own mesh: this car has none there (1073, 7278). They
    # sit in a shallow recess; everything standing out of its floor there
    # is pressed back down onto it (the floor: the deepest level there).
    trim = {i for i, s in enumerate(car.material_slots) if s.material.name.startswith(BODY + "Coloured")}
    letters = {v for f in bm.faces if f.material_index in trim for v in f.verts if v.co.x < 0.4 and abs(v.co.y) < GT_LETTERS[0] and GT_LETTERS[1] < v.co.z < GT_LETTERS[2]}
    floor = sorted(v.co.x for v in letters)[int(len(letters) * 0.9)]
    for v in letters:
        v.co.x = max(v.co.x, floor)
    bmesh.ops.delete(bm, geom=gone, context="FACES")

    # The model's rear spoiler: loose pieces standing on the trunk lid.
    def on_trunk(co):
        return co.x < SPOILER_END and co.z > SPOILER_LOW and abs(co.y) < 1.0

    bmesh.ops.delete(bm, geom=pieces(bm, on_trunk), context="FACES")
    bm.normal_update()
    bm.to_mesh(car.data)
    bm.free()
    car.name = "mustang"
    return car


# The model's rear spoiler: loose pieces behind SPOILER_END (metres from
# the rear bumper) standing above SPOILER_LOW (metres up).
SPOILER_END = 0.6
SPOILER_LOW = 0.92


def pieces(bm, inside, most=6000):
    """the faces of every piece of `bm` (faces joined edge to edge to each
    other and to nothing else) lying wholly where `inside` holds of a
    point, with fewer than `most` faces."""
    seen, found = set(), []
    for f in bm.faces:
        if f in seen or not all(inside(v.co) for v in f.verts):
            continue
        piece, todo = {f}, [f]
        while todo and len(piece) < most:
            for e in todo.pop().edges:
                for g in e.link_faces:
                    if g not in piece:
                        piece.add(g)
                        todo.append(g)
        seen |= piece
        if len(piece) < most and all(inside(v.co) for g in piece for v in g.verts):
            found += piece
    return found


def hit(car, origin, direction):
    """where a ray from `origin` along `direction` first meets the car."""
    from mathutils import Vector

    found, where, _, _ = car.ray_cast(Vector(origin), Vector(direction))
    if not found:
        sys.exit(f"ERROR: the mustang: nothing of the car from {origin} along {direction}")
    return where


def side_at(car, x, z, side):
    """how far out the car's `side` (-1 or 1) is at `x`, `z` metres up."""
    return abs(hit(car, (x, side * 3.0, z), (0.0, -side, 0.0)).y)


def top_at(car, x, across):
    """how high the car's top is at `x`, `across` metres from the middle."""
    return hit(car, (x, across, 3.0), (0.0, 0.0, -1.0)).z


def front_at(car, across, z):
    """how far forward the car's nose is `across` from the middle, `z` up."""
    return hit(car, (MUSTANG_LENGTH + 1.0, across, z), (-1.0, 0.0, 0.0)).x


def rear_at(car, across, z):
    """how far back the car's tail is `across` from the middle, `z` up."""
    return hit(car, (-1.0, across, z), (1.0, 0.0, 0.0)).x


def windshield(car):
    """where the windshield meets the hood (its foot, the front most of the
    glass above the lights) and where it meets the roof (its top)."""
    glass = {i for i, s in enumerate(car.material_slots) if s.material.name.startswith(GLASS)}
    screen = [p for p in car.data.polygons if p.material_index in glass and p.center.z > 0.9 and p.center.x > MUSTANG_LENGTH / 2]
    return max(p.center.x for p in screen), min(p.center.x for p in screen if p.center.z > 1.2)


def recolour(car, mats, cowl):
    """this car's own colours on the model, each face found by what it is,
    where it is and which way it faces: the roof, trunk, hood and mirrors
    black, the headlights without their glass and the red taillight glass.
    `mats` is the black, the red and the tinted glass; `cowl` is where the
    windshield meets the hood. The door glass is taken off (the windows
    are down in the photos) and the windshield becomes tinted glass the
    inside shows through; this runs last, once nothing more is laid onto
    the glass."""
    slots = {s.material.name: i for i, s in enumerate(car.material_slots)}
    first = len(car.material_slots)
    for m in mats:
        car.data.materials.append(m)
    black_slot, red_slot, tinted_slot = first, first + 1, first + 2
    paint = {i for name, i in slots.items() if name.startswith(PAINT)}
    glass = {i for name, i in slots.items() if name.startswith(GLASS)}
    # Where the door glass starts: the pillar between it and the quarter
    # glass, this far ahead of the rear axle (9131).
    door_glass = tyres(car)[0][0] + DOOR_GLASS_AHEAD
    # Where the rear glass meets the trunk.
    back = min(p.center.x for p in car.data.polygons if p.material_index in glass and p.center.z > 1.0)
    # The paint is sliced along every line a black panel ends on, so each
    # face lies wholly on one side and the edges run clean.
    import bmesh
    import numpy as np

    bm = bmesh.new()
    bm.from_mesh(car.data)
    # The black roof reaches out to the blue roof rails: where they meet at
    # the roof's front corner, read in 3745 and cast onto the body (the
    # left; the right reading's ray passes just outside the body), both
    # sides alike.
    roof_half = abs(measured("roof edge left front")[1])
    cuts = [((0.0, side * roof_half, 0.0), (0.0, 1.0, 0.0)) for side in (-1, 1)]
    # The trunk lid's sides run on a slant, from the rear glass to the tail.
    (x0, y0), (x1, y1) = trunk_edge()
    cuts += [((x0, side * y0, 0.0), (-(y1 - y0), side * (x1 - x0), 0.0)) for side in (-1, 1)]
    # The hood's seams with the fenders, straight between their measured
    # points.
    seam = hood_seam()
    for (x0, y0), (x1, y1) in zip(seam, seam[1:]):
        if x1 > x0:
            cuts += [((x0, side * y0, 0.0), (-(y1 - y0), side * (x1 - x0), 0.0)) for side in (-1, 1)]
    # The hood's front edge, above the grille: read at two places across
    # it in 2428 and cast onto the body.
    hood_front = sum(measured(f"hood front edge {n}")[0] for n in "ab") / 2
    cuts.append(((hood_front, 0.0, 0.0), (1.0, 0.0, 0.0)))
    cuts.append(((cowl, 0.0, 0.0), (1.0, 0.0, 0.0)))
    # Where the trunk lid's black ends at the rear glass.
    cuts.append(((back, 0.0, 0.0), (1.0, 0.0, 0.0)))
    # How low the hood comes at its front corners, down to the headlights:
    # both corners read in 2428 and cast onto the body (0.717 and 0.719 m).
    hood_low = sum(measured(f"hood corner {s}")[2] for s in ("left", "right")) / 2
    cuts.append(((0.0, 0.0, hood_low), (0.0, 0.0, 1.0)))
    # The trunk lid's back face is black down to its bottom edge, above
    # the panel between the taillights (7278, cast onto the body).
    trunk_low = measured("trunk bottom middle")[2]
    cuts.append(((0.0, 0.0, trunk_low), (0.0, 0.0, 1.0)))
    for co, no in cuts:
        faces = [f for f in bm.faces if f.material_index in paint]
        verts = list({v for f in faces for v in f.verts})
        edges = list({e for f in faces for e in f.edges})
        bmesh.ops.bisect_plane(bm, geom=verts + edges + faces, dist=1e-5, plane_co=co, plane_no=no)
    # The headlights' glass is taken off, so the lamps inside show as they
    # do through the clear glass in the photos.
    covers = [f for f in bm.faces if f.material_index in glass and f.calc_center_median().z < 0.9 and f.calc_center_median().x > MUSTANG_LENGTH - 0.7]
    bmesh.ops.delete(bm, geom=covers, context="FACES")
    # The windshield and the door glass, from the pillar ahead of the
    # quarter glass forward: the photos show the cage, dash and driver
    # through them. The quarter and rear glass stay; they are dark.
    panes = [f for f in bm.faces if f.material_index in glass and f.calc_center_median().z > 0.9 and door_glass < f.calc_center_median().x < cowl + 0.05]
    # The door glass (facing sideways) is down in the photos: taken off.
    # The windshield is tinted glass the inside shows through (2428).
    bmesh.ops.delete(bm, geom=[f for f in panes if abs(f.normal.y) > 0.7], context="FACES")
    for f in panes:
        if f.is_valid:
            f.material_index = tinted_slot
    # The mirrors: the painted pieces standing out from the doors below the
    # front of the side glass.
    for f in pieces(bm, lambda co: cowl - 1.0 < co.x < cowl + 0.3 and abs(co.y) > MIRROR_OUT and 0.85 < co.z < 1.3):
        if f.material_index in paint:
            f.material_index = black_slot
    bm.to_mesh(car.data)
    bm.free()
    for p in car.data.polygons:
        c, n = p.center, p.normal
        if p.material_index in glass and c.z < 1.0 and c.x < 0.3:
            p.material_index = red_slot
        elif p.material_index not in paint:
            continue
        # The black roof, trunk and hood each end on a straight line, so
        # their edges run clean. The trunk lid is black all over, down its
        # rounded back edge too.
        elif c.x < back and c.z > trunk_low and abs(c.y) < trunk_half(c.x):
            p.material_index = black_slot
        elif n.z < 0.0:
            continue
        elif c.z > 1.25 and abs(c.y) < roof_half:
            p.material_index = black_slot
        # The hood, out to its seams with the fenders (3745: the fenders'
        # tops stay blue) and down to the headlights at its front corners.
        elif cowl < c.x < hood_front and c.z > hood_low and abs(c.y) < float(np.interp(c.x, *zip(*seam))):
            p.material_index = black_slot




def trunk_lip(car, mat):
    """the raised black lip across the trunk lid's back edge (9131, 7278):
    a thin strip standing up along the lid, following its curve across and
    its back edge, which comes forward toward the sides (the lid's back
    corners, measured, are 0.33 to 0.36 m in from the bumper)."""
    count = 24
    (tail_x, tail_half), _ = trunk_edge()
    half = tail_half - 0.08
    across = [-half + 2 * half * i / count for i in range(count + 1)]
    edge, tops = [], []
    for y in across:
        # The lid's back edge here: walking back along the lid, the last
        # place before its top falls away (more than 2 cm in 1 cm).
        x, z = 0.8, top_at(car, 0.8, y)
        while x > 0.02:
            below = top_at(car, x - 0.01, y)
            if below < z - 0.02:
                break
            x, z = x - 0.01, below
        edge.append(x)
        tops.append(z)

    def evened(row):
        # Across the car (the walk finds the edge to the nearest 1 cm).
        return [sum(row[max(i - 2, 0) : i + 3]) / len(row[max(i - 2, 0) : i + 3]) for i in range(len(row))]

    verts = []
    for y, x, z in zip(across, evened(edge), evened(tops)):
        # Standing on the lid, just in from its edge.
        x += TRUNK_LIP[0] + 0.012
        verts +=[(x, y, z - 0.005), (x, y, z + TRUNK_LIP[1]), (x - 0.012, y, z + TRUNK_LIP[1]), (x - 0.012, y, z - 0.005)]
    faces = [(4 * i + k, 4 * i + (k + 1) % 4, 4 * i + 4 + (k + 1) % 4, 4 * i + 4 + k) for i in range(count) for k in range(4)]
    faces += [(0, 1, 2, 3), tuple(4 * count + k for k in (3, 2, 1, 0))]
    return mesh_object("trunk lip", verts, faces, [mat])


# Where the downloaded body's GT letters are on its rear panel: how far
# across either side of the middle, and from how high to how high.
GT_LETTERS = (0.12, 0.74, 0.90)

# Half a US plate's width (12 inches), the body's plate panel's too.
PLATE_HALF = 0.1525

# How far in from the lid's back edge the lip stands (on it), and how tall
# it is (judged: only 7278 shows it).
TRUNK_LIP = (0.0, 0.03)


def trunk_edge():
    """the trunk lid's side edge, where its black meets the blue quarter
    panel, as two (x, how far out) points, tail first: read in 7278 (the
    left side, near the rear glass and at the taillight) and 1073 (the
    right side at the taillight), each cast onto the body. At the tail the
    two sides are averaged (0.73 m and 0.63 m out: 10 cm apart, so each
    reading is good to about 5 cm)."""
    front, left, right = (measured(f"trunk edge {n}") for n in ("left front", "left rear", "right rear"))
    tail = ((left[0] + right[0]) / 2, (abs(left[1]) + abs(right[1])) / 2)
    return tail, (front[0], abs(front[1]))


def trunk_half(x):
    """how far out the trunk lid reaches at `x`: on the line through its
    measured edge (trunk_edge())."""
    (x0, y0), (x1, y1) = trunk_edge()
    return y0 + (y1 - y0) * (x - x0) / (x1 - x0)
WINDSHIELD = (0.061, 0.273, 0.408)
WINDSHIELD_SOLID = 0.8


def hood_seam():
    """the hood's seams with the fenders, as (x, how far out) from the
    cowl forward: two points on each side, read in 3745 and placed on the
    body (reference.md), both sides together (they agree within 1 cm)."""
    return sorted((x, abs(y)) for x, y, _ in (measured(f"hood seam {side} {end}") for side in ("left", "right") for end in ("back", "front")))
MIRROR_OUT = 0.70
# How far ahead of the rear axle the door glass starts (9131).
DOOR_GLASS_AHEAD = 0.60


def mesh_object(name, verts, faces, mats):
    """an object of `verts` and `faces` in `mats`, its faces all turned
    outward."""
    import bmesh

    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces)
    for m in mats:
        mesh.materials.append(m)
    bm = bmesh.new()
    bm.from_mesh(mesh)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(mesh)
    bm.free()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.collection.objects.link(obj)
    return obj


def bar(name, a, b, size, mat):
    """a flat bar from point `a` to point `b` (x, y, z), `size` (across,
    thick) round it, its across side level."""
    dx, dy, dz = (b[i] - a[i] for i in range(3))
    level = math.hypot(dx, dy)
    return box(name, (math.hypot(level, dz), size[0], size[1]), at=tuple((a[i] + b[i]) / 2 for i in range(3)), rotation=(0, math.atan2(-dz, level), math.atan2(dy, dx)), mat=mat)


def plate(name, outline, at, thick, mat, axis="Y"):
    """a flat plate `thick` through, its `outline` (u, v) points: (x, z) at
    y = `at` when `axis` is Y, (y, z) at x = `at` when X, (x, y) at z =
    `at` when Z."""
    place = {"Y": lambda u, v, w: (u, w, v), "X": lambda u, v, w: (w, u, v), "Z": lambda u, v, w: (u, v, w)}[axis]
    k = len(outline)
    verts = [place(u, v, at - thick / 2) for u, v in outline] + [place(u, v, at + thick / 2) for u, v in outline]
    return mesh_object(name, verts, prism_faces(k), [mat])


def bevel(obj, width, segments=2):
    """`obj` with its edges rounded `width` metres."""
    b = obj.modifiers.new("bevel", "BEVEL")
    b.width = width
    b.segments = segments
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=b.name)
    return obj


# Texas, as (longitude, latitude): the wing's end plates are cut in its shape.
TEXAS = [
    (-103.04, 36.50), (-100.00, 36.50), (-100.00, 34.56), (-99.2, 34.3), (-98.5, 34.1), (-97.1, 33.8), (-96.0, 33.9),
    (-95.2, 33.9), (-94.48, 33.64), (-94.04, 33.55), (-94.04, 31.99), (-93.6, 31.0), (-93.7, 30.3), (-93.84, 29.7),
    (-94.7, 29.35), (-95.1, 29.1), (-96.2, 28.5), (-97.0, 27.9), (-97.4, 27.2), (-97.3, 26.4), (-97.15, 25.95),
    (-97.5, 25.9), (-98.2, 26.1), (-99.1, 26.5), (-99.5, 27.5), (-100.3, 28.3), (-100.7, 29.1), (-101.4, 29.77),
    (-102.3, 29.9), (-102.7, 29.7), (-103.2, 29.0), (-103.9, 29.3), (-104.5, 29.8), (-104.9, 30.6), (-106.0, 31.4),
    (-106.63, 31.99), (-103.06, 32.0),
]
def texas_measured():
    """the plates' Texas as measured (tall, west, level for texas()): fitted
    by least squares to both plates' El Paso tips and panhandle top corners
    together, triangulated from 1073, 7278 and 9131 (reference.md). Both
    plates are the one shape at the one place along and up the car, as the
    wing is built (fitted each from its own points they stood apart)."""
    import numpy as np

    squeeze = math.cos(math.radians(31.0))
    rows, values = [], []
    for side, name, (lon, lat) in ((s, n, ll) for s in ("left", "right") for n, ll in (("el paso", (-106.63, 31.99)), ("panhandle west", (-103.04, 36.5)), ("panhandle east", (-100.0, 36.5)))):
        x, _, z = measured(f"texas {side} {name}")
        # x = west - (lon + 106.63) * squeeze * s;  z = level + (lat - 33) * s
        rows += [[1.0, 0.0, -(lon + 106.63) * squeeze], [0.0, 1.0, lat - 33.0]]
        values += [x, z]
    (west, level, s), *_ = np.linalg.lstsq(np.array(rows), np.array(values), rcond=None)
    return s * (36.5 - 25.9), west, level


def texas(tall, west, level):
    """Texas `tall` metres high, its west end at x = `west`, latitude 33
    at height `level`, north up and west toward the nose."""
    # Reversed, so the outline still runs the same way round once mirrored.
    return [texas_point(lon, lat, tall, west, level) for lon, lat in reversed(TEXAS)]


def texas_point(lon, lat, tall, west, level):
    """where a place on the Texas plate (longitude, latitude) stands, as
    (x, z), for texas()'s `tall`, `west` and `level`."""
    scale = tall / (36.5 - 25.9)
    squeeze = math.cos(math.radians(31.0))
    return west - (lon + 106.63) * squeeze * scale, level + (lat - 33.0) * scale


# Where the plates are fixed to the blade, as places on the Texas plate
# (longitude, latitude): the right plate in 1073, its pixels turned into
# the map by its three measured corners (the panhandle's two top corners,
# El Paso; the plate is flat and seen from far off, so that is exact).
# The two bolts through the plate into the blade, front and back.
TEXAS_BOLTS = ((-99.7, 31.72), (-95.48, 31.99))


# The race numbers' font: Impact, heavy and narrow like the car's 776
# (Windows' own copy, so the models are built on Windows).
NUMBER_FONT = r"C:\Windows\Fonts\impact.ttf"
# How letters are turned to read on each face of the car: its -y side (the
# nose to the right), its +y side (the nose to the left), its rear.
FACING = {-1: (math.pi / 2, 0, 0), 1: (math.pi / 2, 0, math.pi), "rear": (math.pi / 2, 0, -math.pi / 2)}


def measured(name):
    """where one of our parts truly is, triangulated from the photos
    (scripts/mustang-gt/candidate-b/triangulated.json, in the body's frame
    centred along the car), as (x, y, z) with the rear bumper at x = 0."""
    x, y, z = json.loads(MEASURED.read_text())[name]
    return x + MUSTANG_LENGTH / 2, y, z


MEASURED = Path(__file__).resolve().parent / "mustang-gt" / "candidate-b" / "triangulated.json"
# The reference points as they are on the downloaded body (its own frame,
# centred along the car), to measure what the car does differently.
MODEL_POINTS = json.loads((Path(__file__).resolve().parent / "mustang-gt" / "candidate-b" / "modelpoints.json").read_text())
# The car's tyres, top to bottom, in metres: each tyre's height in pixels
# in the square side photos, scaled by the wheelbase between the hubs
# there (Ford: 2.72 m). Rear: 9131 0.631, 7278 0.649; front: 9131 0.625.
TYRE_DIAMETER = {"front": 0.625, "rear": 0.64}


def text(name, words, size, at, facing, mat, squeeze=1.0, onto=None, fit=None):
    """`words` in raised letters `size` high, centred at `at`, on the face
    of the car `facing` (a key of FACING); with `onto` (the car), laid onto
    the curve of that face. With `fit` (across, tall; either None keeps the
    letters' shape), the words are sized to exactly that, in metres."""
    bpy.ops.object.text_add(location=at, rotation=FACING[facing])
    obj = bpy.context.active_object
    obj.data.body = words
    obj.data.font = bpy.data.fonts.load(NUMBER_FONT, check_existing=True)
    obj.data.size = size
    obj.data.extrude = 0.003
    obj.data.align_x = "CENTER"
    obj.data.align_y = "CENTER"
    obj.scale = (squeeze, 1.0, 1.0)
    bpy.ops.object.convert(target="MESH")
    obj = bpy.context.active_object
    obj.name = name
    obj.data.materials.append(mat)
    if fit:
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
        verts = obj.data.vertices
        across = 1 if facing == "rear" else 0
        a0, a1 = min(v.co[across] for v in verts), max(v.co[across] for v in verts)
        z0, z1 = min(v.co.z for v in verts), max(v.co.z for v in verts)
        wide = None if fit[0] is None else fit[0] / (a1 - a0)
        tall = wide if fit[1] is None else fit[1] / (z1 - z0)
        wide = tall if wide is None else wide
        for v in verts:
            v.co[across] = at[across] + (v.co[across] - (a0 + a1) / 2) * wide
            v.co.z = at[2] + (v.co.z - (z0 + z1) / 2) * tall
    if onto:
        lay_onto(obj, onto, facing)
    return obj


# The longest a piece of a laid-on part may be (metres), so it follows the
# body's curve.
LAY_PIECE = 0.03


def lay_onto(obj, car, facing, proud=0.004, lift=0.001):
    """`obj`, a flat part, laid onto the curve of the car's face `facing`
    (a key of FACING): its back `lift` off the surface and its front
    `proud` further out."""
    import bmesh

    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    # Cut into small pieces first, so a long flat face (a 7's stroke) bends
    # with the body's curve instead of sinking into it between its corners.
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bmesh.ops.triangulate(bm, faces=bm.faces)
    for _ in range(6):
        long_edges = [e for e in bm.edges if e.calc_length() > LAY_PIECE]
        if not long_edges:
            break
        bmesh.ops.subdivide_edges(bm, edges=long_edges, cuts=1, use_grid_fill=True)
        bmesh.ops.triangulate(bm, faces=bm.faces)
    bm.to_mesh(obj.data)
    bm.free()
    verts = obj.data.vertices
    if facing == "rear":
        middle = sum(v.co.x for v in verts) / len(verts)
        for v in verts:
            v.co.x = rear_at(car, v.co.y, v.co.z) - lift - (proud if v.co.x < middle else 0.0)
    else:
        middle = sum(v.co.y for v in verts) / len(verts)
        for v in verts:
            out = proud if (v.co.y - middle) * facing > 0 else 0.0
            v.co.y = facing * (side_at(car, v.co.x, v.co.z, facing) + lift + out)


# The wing's blade, as (along from its front edge, up) on a one metre
# chord: arched on top, a gurney flap standing at its back edge.
BLADE = [
    (0.0, 0.0), (0.03, 0.025), (0.12, 0.05), (0.3, 0.065), (0.5, 0.06), (0.75, 0.04), (1.0, 0.012), (1.0, 0.05),
    (0.985, 0.05), (0.985, 0.0), (0.75, -0.005), (0.5, -0.008), (0.25, -0.01), (0.08, -0.012), (0.02, -0.008),
]


def on_hood(car, x, across, lift=0.0):
    """where a point `lift` above the hood at (x, `across`) is, and the
    hood's pitch there, for laying parts flat on it."""
    slope = (top_at(car, x + 0.05, across) - top_at(car, x - 0.05, across)) / 0.1
    return (x, across, top_at(car, x, across) + lift), -math.atan(slope)


def vent(car, corners, mats):
    """a flat vent in the hood, as in 1164 and 3745: a dark four sided
    opening lying on the hood between its `corners`, (x, y) each, in the
    order back one side, back other side, front other side, front one
    side; a raised lip along its front edge; rivets round it."""
    well, slat, rivet = mats
    (bx0, by0), (bx1, by1), (fx1, fy1), (fx0, fy0) = corners

    def at(u, v, lift=0.0):
        """the point on the hood `u` along the vent (-1 its back, 1 its
        front) and `v` across it (-1 to 1), `lift` above the hood."""
        s, t = (u + 1.0) / 2.0, (v + 1.0) / 2.0
        x = (1 - s) * ((1 - t) * bx0 + t * bx1) + s * ((1 - t) * fx0 + t * fx1)
        y = (1 - s) * ((1 - t) * by0 + t * by1) + s * ((1 - t) * fy0 + t * fy1)
        return on_hood(car, x, y, lift)[0]

    corners = ((-1, -1), (-1, 1), (1, 1), (1, -1))
    panel = [at(u, v, 0.006) for u, v in corners] + [at(u, v, -0.006) for u, v in corners]
    parts = [mesh_object("vent", panel, prism_faces(4), [well])]
    # The lip along the front edge, standing a little higher.
    lip = [at(1.0, v, h) for h in (0.004, 0.02) for v in (-1.0, 1.0)]
    lip += [(px + 0.012, py, pz) for px, py, pz in lip]
    lip = [lip[i] for i in (0, 1, 3, 2, 4, 5, 7, 6)]
    parts.append(mesh_object("vent lip", lip, prism_faces(4), [slat]))
    around = [(u, v) for u in (-1, 1) for v in [k / 4 - 1 for k in range(9)]] + [(v, u) for u in (-1, 1) for v in [k / 3 - 1 for k in range(1, 6)]]
    for u, v in around:
        parts.append(cylinder("rivet", 0.005, 0.008, sides=8, at=at(u * 1.08, v * 1.12, 0.002), mat=rivet))
    return parts


def sticker(car, name, tall, mat):
    """a sticker on the glass from its two ends as measured, `tall` metres
    across the other way, lying on the glass's slope."""
    from mathutils import Vector

    a, b = Vector(measured(f"{name} left end")), Vector(measured(f"{name} right end"))
    middle = Vector(measured(f"{name} middle"))
    along = (b - a).normalized()
    # Across the sticker: up the glass, square to its length.
    up = Vector((0.0, 0.0, 1.0))
    slope = (top_at(car, middle.x - 0.05, middle.y) - top_at(car, middle.x + 0.05, middle.y)) / 0.1
    up_glass = Vector((-1.0, 0.0, slope)).normalized()
    across = (up_glass - along * up_glass.dot(along)).normalized() * (tall / 2)
    lift = along.cross(across).normalized() * 0.002
    if lift.z < 0:
        lift = -lift
    corners = [a - across, b - across, b + across, a + across]
    verts = [tuple(c + lift * 0.5) for c in corners] + [tuple(c + lift * 1.5) for c in corners]
    return mesh_object(name, verts, prism_faces(4), [mat])


def prism_faces(k):
    """the faces of a solid made of a `k` sided outline on top (its first
    `k` points) and the same outline below it (the next `k`)."""
    faces = [tuple(range(k)), tuple(range(2 * k - 1, k - 1, -1))]
    return faces + [(i, (i + 1) % k, k + (i + 1) % k, k + i) for i in range(k)]


def scoop(car, ramp, back, front, inner, outer, side, mat):
    """a raised louvred scoop at mid hood (9131, 7278, 1073), from `inner`
    to `outer` out on `side`: a ramp rising from the hood at `ramp` to the
    fins' height at `front`, hiding them from ahead, and black fins
    standing up across it from `front` back to `back`, leaning back."""
    parts = []
    pieces = 6
    step = (outer - inner) / pieces
    # The ramp: a strip across the scoop, from the hood up to the fins'
    # height, following the hood's curve across.
    verts = []
    for k in range(pieces + 1):
        y = side * (inner + step * k)
        verts += [on_hood(car, ramp, y, 0.0)[0], on_hood(car, front, y, SCOOP_TALL)[0]]
    faces = [(2 * k, 2 * k + 2, 2 * k + 3, 2 * k + 1) for k in range(pieces)]
    parts.append(mesh_object("scoop ramp", verts, faces, [mat]))
    # Each fin in short pieces across, each standing on the hood, so the
    # fin follows the scoop's curve.
    for i in range(SCOOP_FINS):
        x = back + (front - back) * (i + 0.5) / SCOOP_FINS
        for k in range(pieces):
            (fx, fy, fz), pitch = on_hood(car, x, side * (inner + step * (k + 0.5)), SCOOP_TALL / 2)
            parts.append(box("louvre", (0.01, step + 0.002, SCOOP_TALL), at=(fx, fy, fz), rotation=(0, pitch - 0.6, 0), mat=mat))
    return parts


# How many fins a scoop has, and how tall they stand above the hood: 9131
# shows both side vents' lips side on, ten openings each, standing 27
# pixels (3.1 cm at 9131's 1.14 mm a pixel, from the door's 7: 212 pixels
# for 0.242 m).
SCOOP_FINS = 10
SCOOP_TALL = 0.031


def tyres(car):
    """the rear and the front axle's tyres, measured from the model: where
    along the car the axle is, how high the tyres' middle stands, their
    radius, and how far out their outer faces are."""
    tyre = {i for i, s in enumerate(car.material_slots) if s.material.name.startswith(TYRE)}
    found = []
    for front in (False, True):
        at = [p.center for p in car.data.polygons if p.material_index in tyre and (p.center.x >= MUSTANG_LENGTH / 2) == front]
        x0, x1 = min(c.x for c in at), max(c.x for c in at)
        z0, z1 = min(c.z for c in at), max(c.z for c in at)
        found.append(((x0 + x1) / 2, (z0 + z1) / 2, (z1 - z0) / 2, max(abs(c.y) for c in at)))
    return found


def beam(name, a, b, wide, thick, mat):
    """a bar from point `a` to point `b`, `wide` across in the plane of a
    wheel (upright, along the car) and `thick` across that."""
    from mathutils import Vector

    a, b = Vector(a), Vector(b)
    along = (b - a).normalized()
    flat = along.cross(Vector((0.0, 1.0, 0.0))).normalized() * (wide / 2)
    deep = along.cross(flat).normalized() * (thick / 2)
    verts = [tuple(p + s * flat + t * deep) for p in (a, b) for s, t in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
    faces = [(0, 1, 2, 3), (7, 6, 5, 4), (0, 4, 5, 1), (1, 5, 6, 2), (2, 6, 7, 3), (3, 7, 4, 0)]
    return mesh_object(name, verts, faces, [mat])


def rim(x, z, radius, face, side, mats):
    """this car's black wheel in a tyre of `radius` at axle `x`, its middle
    `z` up, the tyre's outer face `face` out on `side`, as in the photos: a
    round lip, ten single spokes curving from the lip deep into the hub,
    an open middle showing the hub's five nuts, and the brake disc behind
    the spokes (the caliper is the model's own)."""
    black, disc, nut = mats
    r = radius * RIM_MEASURED
    y = side * face
    bpy.ops.mesh.primitive_torus_add(major_radius=r - 0.01, minor_radius=0.012, major_segments=64, location=(x, y - side * 0.012, z), rotation=(math.pi / 2, 0, 0))
    parts = [
        _named("rim lip", black),
        cylinder("hub", 0.075, 0.03, sides=24, at=(x, y - side * (RIM_DISH + 0.015), z), along="Y", mat=black),
        cylinder("brake disc", r - 0.06, 0.03, sides=48, at=(x, y - side * (RIM_DISH + 0.07), z), along="Y", mat=disc),
    ]
    for i in range(5):
        a = i * math.tau / 5
        parts.append(cylinder("nut", 0.011, 0.024, sides=6, at=(x + math.cos(a) * 0.05, y - side * RIM_DISH, z + math.sin(a) * 0.05), along="Y", mat=nut))
    for i in range(10):
        a = i * math.tau / 10
        # Each spoke starts at the lip and curves deep into the hub: two
        # straight lengths, the outer one shallow, the inner one steep.
        lip = (x + math.cos(a) * (r - 0.02), y - side * 0.02, z + math.sin(a) * (r - 0.02))
        bend = (x + math.cos(a) * r * 0.55, y - side * 0.04, z + math.sin(a) * r * 0.55)
        hub = (x + math.cos(a) * 0.07, y - side * RIM_DISH, z + math.sin(a) * 0.07)
        parts.append(beam("spoke", lip, bend, 0.03, 0.02, black))
        parts.append(beam("spoke", bend, hub, 0.03, 0.02, black))
    return parts


# Where the downloaded model's tyre ends and its rim begins, as a share of
# the tyre's radius (to keep its tyre and drop its rim).
RIM_OF_TYRE = 0.79
# This car's rim's radius as a share of its tyre's: the rim's lip against
# the tyre in the side photos, 432 of 513 pixels in 7278, 450 of 540 in
# 9131.
RIM_MEASURED = 0.835
# How deep the rim's dish is: how far in from the tyre's face the hub sits.
RIM_DISH = 0.08


def mustang():
    """the operator's Mustang: Jesterz86's car in this car's colours, and on
    it what the photos show: the black roof, the black hood with its three
    riveted vents, black mirrors, the wing on Texas end plates, the
    splitter on its rods, the red tow hook, the side skirts, the numbered
    sticker, and 776 on both doors and the back. Every black part is the
    one black."""
    black = material("black", MUSTANG_BLACK)
    steel = material("steel", (0.60, 0.60, 0.62))
    disc = material("brake disc", (0.30, 0.30, 0.32))
    hook = material("hook", (0.85, 0.10, 0.10))
    badge = material("badge", (0.45, 0.45, 0.47))
    yellow = material("sticker", (0.95, 0.80, 0.10))
    white = material("white", (0.90, 0.90, 0.88))
    blue = material("paint", MUSTANG_COLOURS[PAINT][0])
    car = mustang_car()
    red = material("taillight glass", (0.80, 0.06, 0.06))
    cowl, brow = windshield(car)
    axles = tyres(car)
    rear_axle, front_axle = axles[0][0], axles[1][0]
    # The wing: the Texas plates as measured, and the blade from its own
    # front corners where it meets the left plate (9131, 2428): its front
    # edge's top, and its chord running back from there.
    texas_tall, _, _ = texas_measured()
    # The blade, from its front edge's top at the left plate (9131, 2428)
    # back to where its top runs into the plates' rear edges (1073, 7278):
    # that is its depth (0.40 m; the bolts through the plates, 1073, lie
    # inside it) and, its back edge being the raised one (tilt, the gurney
    # flap), its height.
    at_left, at_right = measured("blade top at left plate"), measured("blade top at right plate")
    front = measured("wing left end top front")[0]
    back = (at_left[0] + at_right[0]) / 2
    chord = front - back
    wing_middle = (front + back) / 2
    tilt = math.tan(0.14)
    gurney = max(v for u, v in BLADE if u == 1.0)
    wing_height = (at_left[2] + at_right[2]) / 2 - gurney * chord - 0.5 * chord * tilt
    # The plates hang on the blade where they are bolted to it: the two
    # bolts, measured on the right plate in 1073 (cast onto its plane), at
    # their places on the Texas plate (TEXAS_BOLTS). Their size is as
    # measured (texas_measured()).
    bolts = [measured(f"plate bolt {which}") for which in ("front", "back")]
    at_zero = [texas_point(lon, lat, texas_tall, 0.0, 0.0) for lon, lat in TEXAS_BOLTS]
    texas_west = sum(b[0] - p[0] for b, p in zip(bolts, at_zero)) / 2
    texas_level = sum(b[2] - p[1] for b, p in zip(bolts, at_zero)) / 2
    # The blade's underside, how high at a place along the car: its back
    # edge is raised (tilt), so the uprights' tops meet it at different
    # heights.
    underside = sorted([(0.0, 0.0)] + BLADE[BLADE.index((0.985, 0.0)) :])

    def under_blade(x):
        u = min(max((wing_middle + chord / 2 - x) / chord, 0.0), 0.985)
        (u0, v0), (u1, v1) = next((a, b) for a, b in zip(underside, underside[1:]) if a[0] <= u <= b[0])
        v = v0 + (v1 - v0) * (u - u0) / (u1 - u0)
        return wing_height + v * chord + (u - 0.5) * chord * tilt
    # How far out the plates stand: where the blade meets each plate, which
    # 3745 sees from the front (the Texas points come from side and rear
    # views only, which cannot place a part across the car).
    plate_out = (abs(measured("blade top at left plate")[1]) + abs(measured("blade top at right plate")[1])) / 2
    parts = [car]
    for x, z, radius, face in axles:
        for side in (-1, 1):
            parts += rim(x, z, radius, face, side, (black, disc, steel))
    # The row of five small white racetrack outlines on the rear glass, on
    # the car's right, each placed on the glass from 1073 and as wide as the
    # first one measured there; their outlines are too small to trace, so
    # each is a loop.
    a, b = measured("track sticker 1 left end"), measured("track sticker 1 right end")
    wide = math.dist(a, b)
    for i in range(1, 6):
        x, y, z = measured(f"track sticker {i}")
        bpy.ops.mesh.primitive_torus_add(major_radius=wide / 2, minor_radius=0.003, major_segments=24, minor_segments=4, location=(x, y, z + 0.004))
        parts.append(_named("track sticker", white))
    # The fuel door, round, on the car's left behind the rear wheel, as far
    # back and as high as in the photos.
    x, _, z = measured("fuel door middle left")
    out = side_at(car, x, z, 1)
    seam = cylinder("fuel door seam", 0.088, 0.004, sides=48, at=(x, out, z), along="Y", mat=black)
    lay_onto(seam, car, 1, proud=0.001)
    door = cylinder("fuel door", 0.082, 0.004, sides=48, at=(x, out, z), along="Y", mat=blue)
    # Clear of the seam under it (3 mm), so the seam does not show through
    # the door from far off.
    lay_onto(door, car, 1, proud=0.001, lift=0.005)
    parts += [seam, door]
    nose = front_at(car, 0.0, 0.45)
    # The splitter hangs just under the front lip. Its front edge follows
    # the bumper's curve out past it, evened out, and only ever sweeps back
    # toward the corners, as in the photos.
    lip = hit(car, (nose - 0.1, 0.0, -1.0), (0.0, 0.0, 1.0)).z - 0.008
    across = [i * 0.05 - 0.8 for i in range(33)]
    bumper = [front_at(car, y, lip + 0.03) for y in across]
    even = [sum(bumper[max(i - 4, 0) : i + 5]) / len(bumper[max(i - 4, 0) : i + 5]) for i in range(len(bumper))]
    middle = len(even) // 2
    for i in range(middle + 1, len(even)):
        even[i] = min(even[i], even[i - 1])
    for i in range(middle - 1, -1, -1):
        even[i] = min(even[i], even[i + 1])
    # How far it reaches past the bumper: its front tip as measured (9131,
    # 7278) against the bumper's middle.
    reach = measured("splitter front tip")[0] - even[middle]
    edge = [(x + reach, y) for x, y in zip(even, across)]

    for side in (-1, 1):
        # 776 on both doors, as far ahead of the rear wheel as in the photos.
        # As measured in 9131 and 7278: from the first 7's top left corner
        # to the 6's right edge, and the 7's top down to its foot.
        # Each door's own number: the right one measured in 1073 and 3661.
        door = "776 " if side > 0 else "776 right "
        first, foot, six = measured(door + "first 7 top left"), measured(door + "first 7 foot"), measured(door + "6 top right")
        x, z = (first[0] + six[0]) / 2, (first[2] + foot[2]) / 2
        parts.append(text("number", "776", 0.46, (x, side * side_at(car, x, z, side), z), side, black, onto=car, fit=(abs(first[0] - six[0]), first[2] - foot[2])))
        # The black 5.0 badge on the fender ahead of the door, level with
        # the top of the wheel arch.
        # As measured in 9131 and 7278: its two ends.
        a, b = measured("5.0 front end"), measured("5.0 rear end")
        x, z = (a[0] + b[0]) / 2, (a[2] + b[2]) / 2
        # Its height from the 0's top and bottom (9131, 7278).
        tall = measured("5.0 zero top")[2] - measured("5.0 zero bottom")[2]
        parts.append(text("badge", "5.0", 0.077, (x, side * side_at(car, x, z, side), z), side, black, onto=car, fit=(a[0] - b[0], tall)))
        # The side skirts, a black blade between the wheels: its two ends
        # along the car as measured (9131, 7278), hung under the sill (the
        # ends' heights read from the side put it 5 to 7 cm under the
        # body, the height being what side views fix worst), 5 cm deep.
        front, rear = measured("skirt left front top")[0], measured("skirt left rear top")[0]
        middle = (front + rear) / 2
        out = side_at(car, middle, 0.30, side)
        sill = hit(car, (middle, side * (out - 0.04), -1.0), (0.0, 0.0, 1.0)).z
        parts.append(bevel(box("skirt", (front - rear, 0.07, 0.05), at=(middle, side * (out + 0.01), sill - 0.02), mat=black), 0.01))

        # The wing's uprights, as in the photos (1073, 7278): an N standing
        # on the trunk. A base bar along the trunk bolted down at both ends;
        # from the back foot a post nearly straight up to the blade and a
        # back leg leaning back to the blade's back edge; from the front
        # foot a brace up and back to the post's top. A flat plate under the
        # blade joins the three tops and is bolted to it.
        # Each side's feet as measured (the right from 1073 and 3661, the
        # left from 1073 on the plane the right fixes across the car); the
        # tops, hidden behind the right plate, from the left's (1073).
        own = "left" if side > 0 else "right"
        post_foot, brace_foot = measured(f"upright {own} post foot"), measured(f"upright {own} brace foot")
        # The front bar runs from the front foot up to the blade; the
        # crossbar from the back foot up to the front bar, joining it about
        # three quarters of the way up (7237, against the sky); the back bar
        # from the back foot up, leaning back, to the blade. In 1073 the
        # bars above the joint are lost against the far Texas plate: what
        # was read there is the joint (the "post top") and a point on the
        # back bar ("back top"). Each bar goes on in the line those points
        # give it until it meets the blade's underside.
        joint, back_through = measured("upright left post top"), measured("upright left back top")
        y = side * abs(post_foot[1])

        def to_blade(start, through):
            """from `start` on through `through` (x, z) until the blade."""
            (x0, z0), (x1, z1) = start, through
            k = 1.0
            while z0 + (z1 - z0) * k < under_blade(x0 + (x1 - x0) * k) and k < 5.0:
                k += 0.01
            return (x0 + (x1 - x0) * k, y, z0 + (z1 - z0) * k)

        front_foot = (brace_foot[0], brace_foot[2])
        back_foot = (post_foot[0], post_foot[2])
        joint_at = (joint[0], y, joint[2])
        parts.append(bar("upright base", (front_foot[0], y, front_foot[1]), (back_foot[0], y, back_foot[1]), (0.01, 0.03), black))
        parts.append(bar("upright front bar", (front_foot[0], y, front_foot[1]), to_blade(front_foot, (joint[0], joint[2])), (0.01, 0.035), black))
        parts.append(bar("upright crossbar", (back_foot[0], y, back_foot[1]), joint_at, (0.01, 0.03), black))
        parts.append(bar("upright back bar", (back_foot[0], y, back_foot[1]), to_blade(back_foot, (back_through[0], back_through[2])), (0.01, 0.035), black))
        for foot in (post_foot, brace_foot):
            parts.append(bevel(box("foot", (0.06, 0.05, 0.01), at=(foot[0], y, foot[2] - 0.005), mat=black), 0.003))
        # The wing's end plates, cut in the shape of Texas, bolted to the
        # blade.
        # Both plates are the same shape, El Paso toward the nose (9131,
        # 1073): the right one reads mirrored from its own side.
        # Both plates hung on the blade where 1073 shows them fixed, with
        # the bolts where they are on the plate.
        outline = texas(texas_tall, texas_west, texas_level)
        parts.append(plate("end plate", outline, side * plate_out, 0.006, black))
        for lon, lat in TEXAS_BOLTS:
            x, z = texas_point(lon, lat, texas_tall, texas_west, texas_level)
            parts.append(cylinder("bolt", 0.008, 0.012, sides=8, at=(x, side * (plate_out + 0.005), z), along="Y", mat=steel))

        # The splitter's end plates, and the rods holding its front edge up
        # to the bumper.
        # Each end plate stands on the back part of the splitter's side
        # edge, from its corners as measured (9131, 7278; the right one the
        # mirror of the left).
        start, end = measured("splitter end left top rear")[0], measured("splitter end left top front")[0]
        top, bottom = measured("splitter end left top rear")[2], measured("splitter end left bottom rear")[2]
        # Across: from the corner seen in three photos (9131, 7278, 2428).
        out = measured("splitter end left top front")[1]
        parts.append(bevel(box("splitter end", (end - start, 0.012, top - bottom), at=((start + end) / 2, side * out, (top + bottom) / 2), mat=black), 0.004))
        # The rods, each from its foot on the splitter's front edge up and
        # back to the bumper, the outer ones leaning in (2428: both ends of
        # each read; the tops cast onto the bumper, the feet onto the
        # upright plane at the splitter's front; both sides together).
        for which in ("inner", "outer"):
            foot = [measured(f"rod {s} {which} foot") for s in ("left", "right")]
            top = [measured(f"rod {s} {which} top") for s in ("left", "right")]
            out = (abs(foot[0][1]) + abs(foot[1][1])) / 2
            x = next(x for x, y in edge if abs(y - side * out) < 0.026) - 0.03
            up = [sum(t[i] for t in top) / 2 for i in (0, 2)]
            top_out = (abs(top[0][1]) + abs(top[1][1])) / 2
            parts.append(bar("splitter rod", (x, side * out, lip), (up[0], side * top_out, up[1]), (0.008, 0.008), steel))

    # The wing's blade, its back edge raised (tilt).
    blade = [(wing_middle + chord / 2 - u * chord, wing_height + v * chord + (u - 0.5) * chord * tilt) for u, v in BLADE]
    parts.append(plate("wing", blade, 0.0, 2 * plate_out, black))
    parts.append(trunk_lip(car, black))
    # The small 776 on the back, just left of the plate (7278: the 6 ends
    # at the plate's edge): its height from the 7's top and the 6's bottom
    # placed on the body from 7278 (0.10 m), its width the font's own for
    # that height. Across the car the rear is only seen at a steep angle
    # (7278, 1073), which puts it 0.43 m wide; not used.
    a, b = measured("rear 776 7 top left"), measured("rear 776 6 bottom right")
    z, tall = (a[2] + b[2]) / 2, a[2] - b[2]
    number = text("rear number", "776", 0.16, (rear_at(car, PLATE_HALF + 0.12, z), PLATE_HALF + 0.12, z), "rear", black, fit=(None, tall))
    shift = PLATE_HALF + 0.02 - min(v.co.y for v in number.data.vertices)
    for v in number.data.vertices:
        v.co.y += shift
    lay_onto(number, car, "rear")
    parts.append(number)
    # The windshield's two stickers, as measured on the glass (2428).
    parts.append(sticker(car, "3 sticker", 0.035, yellow))
    parts.append(sticker(car, "inspection sticker", 0.035, white))
    # The hood, as measured (reference.md, "The front: where each part is
    # and how big"): H its half width, L its length from the windshield's
    # foot to its front edge. Three vents, the side ones with a raised
    # louvred front lip (1164).
    # Each vent from its four corners as measured: the middle one
    # triangulated (1164, 2428), the side ones cast onto the hood from 1164
    # (both sides' readings averaged, mirrored; they agree within 3 to 9
    # cm). The side vents reach 15 cm further back than the middle one,
    # their outer back corner 12 cm behind the inner.
    def flat(name):
        x, y, _ = measured(name)
        return x, y

    parts += vent(car, [flat(f"middle vent {c}") for c in ("back right", "back left", "front left", "front right")], (black, black, badge))
    side_corner = {}
    for c in ("back inner", "back outer", "front inner", "front outer"):
        a, b = flat(f"side vent left {c}"), flat(f"side vent right {c}")
        side_corner[c] = ((a[0] + b[0]) / 2, (abs(a[1]) + abs(b[1])) / 2)
    for side in (-1, 1):
        parts += vent(car, [(x, side * y) for x, y in (side_corner[c] for c in ("back inner", "back outer", "front outer", "front inner"))], (black, black, badge))
        # Each side vent's raised front lip, its louvre fins facing back
        # (the fins seen from the side in 9131): the vent's front third.
        front = (side_corner["front inner"][0] + side_corner["front outer"][0]) / 2
        back = (side_corner["back inner"][0] + side_corner["back outer"][0]) / 2
        inner, outer = side_corner["front inner"][1], side_corner["front outer"][1]
        parts += scoop(car, front + 0.04, front - (front - back) / 3, front, inner, outer, side, black)
    # The plate on the body's own plate panel in the middle of the rear
    # (0.30 m across, raised 10 cm), a US plate's size (12 by 6 inches);
    # its height from its four corners as measured in 1073 and 7278 (their
    # places across the car, seen only at a steep angle, came out 0.48 m
    # apart, ±7 to 9 cm each; not used).
    corners = [measured(f"plate {a} {b}") for a in ("top", "bottom") for b in ("left", "right")]
    y = 0.0
    z = sum(c[2] for c in corners) / 4
    wide, tall = 2 * PLATE_HALF, 0.152
    # Leaning as the bumper does there, from its top to its bottom.
    top, bottom = rear_at(car, y, z + tall / 2), rear_at(car, y, z - tall / 2)
    back = (top + bottom) / 2
    lean = math.asin((top - bottom) / tall)
    parts.append(bevel(box("plate", (0.012, wide, tall), at=(back - 0.008, y, z), rotation=(0, lean, 0), mat=black), 0.004))
    parts.append(box("plate edge", (0.006, wide + 0.02, tall + 0.02), at=(back - 0.003, y, z), rotation=(0, lean, 0), mat=white))
    # TEXAS across its top and MUSTANG on its frame; its own letters are
    # too blurred in the photos to read.
    for words, size, up in (("TEXAS", 0.024, 0.4), ("MUSTANG", 0.016, -0.55)):
        at = z + tall * up
        parts.append(text("plate " + words.lower(), words, size, (back + (top - bottom) * up - 0.017, y, at), "rear", white))
    # The red tow hook on its bracket on the bumper bar above the lower
    # intake, right of the middle: its bolt placed on the body from 2428.
    _, y, z = measured("tow hook bracket bolt")
    face = front_at(car, y, z)
    parts.append(bevel(box("hook bracket", (0.06, 0.04, 0.04), at=(face + 0.02, y, z), mat=black), 0.005))
    parts.append(cylinder("hook bolt", 0.014, 0.012, sides=16, at=(face + 0.055, y, z), along="X", mat=steel))
    bpy.ops.mesh.primitive_torus_add(major_radius=0.028, minor_radius=0.007, location=(face + 0.055, y, z - 0.045), rotation=(0, math.pi / 2, 0))
    parts.append(_named("tow hook", hook))
    # The splitter under it all, its sides running straight back from the
    # ends of its front edge toward the front wheels.
    # Its sides reach out to its end plates, as measured.
    # Its sides reach out to its end plates and back to their rear, as
    # measured.
    half = measured("splitter end left top front")[1]
    back = measured("splitter end left top rear")[0]
    splitter = [(back, -half), (edge[0][0] - 0.06, -half), *edge, (edge[-1][0] - 0.06, half), (back, half)]
    parts.append(plate("splitter", splitter, lip, 0.015, black, axis="Z"))
    # Last, once nothing more is laid onto the car: its own colours, and
    # the glass taken off.
    # The windshield as it looks from ahead and to the side (2428, 1164):
    # the median inside its four corners, sRGB (31, 72, 106) and (0, 67,
    # 102), blue with the cabin faint behind it. From above (3745) it
    # shows the sky instead, which the model does not draw. How solid it
    # is set so the model's windshield comes out that colour in 2428.
    tinted = material("windshield", WINDSHIELD, clear=WINDSHIELD_SOLID)
    recolour(car, (black, red, tinted), cowl)
    car = join(parts, "mustang")
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(40))
    # Where join() moved the car to stand it on its ground point, for
    # turning places in the exported model back to the body's own frame
    # (reference.md, "How the model is matched to the photos").
    print(f"mustang: moved by {tuple(round(c, 4) for c in car.location)} from its rear bumper at x = 0")
    return car


# -- buildings -----------------------------------------------------------------
#
# A building is drawn a tile at a time (src/structure.rs): every piece is
# one tile (a metre) across, near white, and the game tints it the
# building's colour. Heights as src/structure.rs stands them (WALL_HEIGHT,
# FURNITURE_HEIGHT); change both together.
WALL_HEIGHT = 3.0
FURNITURE_HEIGHT = 0.9
LIGHT = (0.92, 0.92, 0.92)


def wall():
    """one tile of wall: a block WALL_HEIGHT tall."""
    return join([box("block", (1.0, 1.0, WALL_HEIGHT), mat=material("wall", LIGHT))], "wall")


def furniture():
    """one tile of furniture (shelves, a counter): a block a little smaller
    than its tile."""
    return join([box("block", (0.9, 0.9, FURNITURE_HEIGHT), mat=material("furniture", LIGHT))], "furniture")


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


def steel_door():
    """one tile of a closed steel door, the bunker's (design.md "Built in
    1 m blocks, doors as blocks"): a grey steel panel across the tile, as
    tall as a wall, with raised bands across it and a heavy lever handle.
    Made the wooden door's size; the game stretches both to the doorway."""
    steel = material("steel", (0.42, 0.44, 0.46))
    band = material("band", (0.3, 0.32, 0.34))
    metal = material("handle", (0.7, 0.7, 0.68))
    parts = [box("panel", (1.0, 0.12, WALL_HEIGHT), at=(0, 0, WALL_HEIGHT / 2), mat=steel)]
    for i, z in enumerate((0.15, 0.5, 0.85)):
        parts.append(box(f"band_{i}", (0.96, 0.15, 0.08), at=(0, 0, WALL_HEIGHT * z), mat=band))
    parts.append(box("handle", (0.2, 0.18, 0.05), at=(0.32, 0, WALL_HEIGHT * 0.5), mat=metal))
    return join(parts, "steel door")


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
# People are built from pieces (design.md "What 2D changes"): one model per
# body, `person <body>`, holding its rig, its animations, and every piece
# that fits it (the body, each hairstyle, the beard, what can be worn), each
# a mesh named after its layer (`body`, `buzzed`, `road sign vest`; a piece
# of several meshes `buzzed.1` and on). The game shows a person's own pieces
# and hides the rest, and plays the animations live.
# The rig is Quaternius's Universal Base Characters (CC0,
# scripts/quaternius-characters/License_Standard.txt), from
# https://quaternius.itch.io/universal-base-characters.
CHARACTERS = Path(__file__).resolve().parent / "quaternius-characters"
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
# The animations a person plays (src/people.rs names them the same):
# standing and walking.
KEPT_ANIMATIONS = ["Idle_Loop", "Walk_Loop"]


def animations(rig):
    """bring in the library's animations the game plays, on `rig`, and
    nothing else of the library (not its mannequin, not the rest)."""
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=str(ANIMATIONS))
    for o in set(bpy.data.objects) - before:
        bpy.data.objects.remove(o, do_unlink=True)
    for act in list(bpy.data.actions):
        if act.name not in KEPT_ANIMATIONS:
            bpy.data.actions.remove(act)
    # Each kept animation on its own track of the rig, so the exporter
    # writes every one of them.
    rig.animation_data_create()
    for name in KEPT_ANIMATIONS:
        act = bpy.data.actions[name]
        track = rig.animation_data.nla_tracks.new()
        track.name = name
        strip = track.strips.new(name, int(act.frame_range[0]), act)
        if act.slots:
            strip.action_slot = act.slots[0]


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


def clothing(body, bones, name, rgb, out):
    """clothes cut from the body itself: a copy of its skin holding only
    what moves with `bones` (a vertex's strongest bone starts with one of
    them), pushed `out` metres off the skin and coloured `rgb`. It keeps
    the body's rig, so it moves as the body does."""
    skin = max(body, key=lambda o: len(o.data.vertices))
    cloth = skin.copy()
    cloth.data = skin.data.copy()
    bpy.context.collection.objects.link(cloth)
    groups = {g.index: g.name for g in cloth.vertex_groups}
    mesh = cloth.data
    gone = []
    for v in mesh.vertices:
        strongest = max(v.groups, key=lambda g: g.weight, default=None)
        bone = groups.get(strongest.group, "") if strongest else ""
        if not bone.startswith(bones):
            gone.append(v.index)
    import bmesh
    bm = bmesh.new()
    bm.from_mesh(mesh)
    bm.verts.ensure_lookup_table()
    bmesh.ops.delete(bm, geom=[bm.verts[i] for i in gone], context="VERTS")
    for v in bm.verts:
        v.co += v.normal * out
    bm.to_mesh(mesh)
    bm.free()
    mat = material(name, rgb)
    mesh.materials.clear()
    mesh.materials.append(mat)
    return [cloth]


# What everyone wears (content/src/lib.rs `worn`): each cut from the body by
# the bones it covers. The shirt runs from the waist to the wrists and the
# neck; the trousers from the waist to the ankles; the boots are the feet.
SHIRT = ("spine", "clavicle", "upperarm", "lowerarm")
TROUSERS = ("pelvis", "thigh", "calf")
BOOTS = ("foot", "ball")

# What people wear, drawn as a layer over the body like a piece: by the
# item's picture name (content/src/lib.rs), built on each body.
WORN = {
    "road sign vest": vest,
    "shirt": lambda rig, body: clothing(body, SHIRT, "shirt", (0.42, 0.4, 0.33), 0.02),
    "trousers": lambda rig, body: clothing(body, TROUSERS, "trousers", (0.25, 0.27, 0.3), 0.012),
    "boots": lambda rig, body: clothing(body, BOOTS, "boots", (0.22, 0.16, 0.11), 0.015),
}


def people():
    """export every body as `person <body>`: its rig with the animations,
    and every piece that fits it, each mesh named after its layer."""
    for body_name, (file, skin, names) in BODIES.items():
        clear()
        rig, body = person(file, skin)
        animations(rig)
        pieces = {n.lower(): piece(n, rig) for n in names}
        worn = {item: build(rig, body) for item, build in WORN.items()}
        for layer, meshes in {"body": body, **pieces, **worn}.items():
            for i, mesh in enumerate(meshes):
                mesh.name = layer if i == 0 else f"{layer}.{i}"
        export(rig, f"person {body_name}")


# Models that export several models themselves.
SETS = {"person": people, "weapons": weapons}


# Items, named as in content/src/lib.rs: each also gets its icon.
ITEMS = {
    "note": note,
    "canned food": lambda: kenney("food", "can", "canned food"),
    "water bottle": lambda: kenney("survival", "bottle", "water bottle"),
    "bandage": bandage,
    "storage box": lambda: kenney("survival", "box", "storage box", DEPLOYABLE),
    "scrap": lambda: kenney("survival", "metal-panel-screws-half", "scrap"),
    "cloth": lambda: kenney("survival", "bedroll-packed", "cloth"),
    "shotgun shell": shotgun_shell,
    "road sign vest": road_sign_vest,
    "shirt": lambda: folded("shirt", (0.42, 0.4, 0.33)),
    "trousers": lambda: folded("trousers", (0.25, 0.27, 0.3)),
    "boots": lambda: folded("boots", (0.22, 0.16, 0.11)),
}

MODELS = {
    **ITEMS,
    "tree": tree,
    "rock": rock,
    # The wasteland's and the ruins' scatter, at their largest side as
    # content/src/lib.rs sizes them.
    "fallen log": lambda: kenney("survival", "tree-log", "fallen log", 1.2, rgb=(0.5, 0.42, 0.32), turn=90),
    "broken wall": lambda: kenney("graveyard", "stone-wall-damaged", "broken wall", 2.0, rgb=(0.45, 0.43, 0.4)),
    # What people eat and drink from where it stands.
    "berry bush": berry_bush,
    "well": well,
    "tap": tap,
    # What flies.
    "pellet": pellet,
    # Buildings, a tile at a time (src/structure.rs).
    "wall": wall,
    "furniture": furniture,
    "floor": floor,
    "door": door,
    "steel door": steel_door,
    # The minor sites' props, named as in content/src/lib.rs.
    # A wrecked car: Kenney's sedan all in rust, lying along x (4.2 m long).
    "car wreck": lambda: kenney("car", "sedan", "car wreck", 4.2, rgb=(0.45, 0.2, 0.15), turn=90),
    "barrel": lambda: kenney("survival", "barrel", "barrel", 0.9),
    "tent": lambda: kenney("survival", "tent", "tent", 2.4),
    "fire pit": lambda: kenney("survival", "campfire-pit", "fire pit", 1.0),
    "crate": lambda: kenney("survival", "box", "crate", 0.5),
    "supply crate": lambda: kenney("survival", "box-large", "supply crate", 1.6),
    "parachute": parachute,
    "gantry": gantry,
    "grave": lambda: kenney("graveyard", "grave", "grave", 1.8),
    # The operator's Mustang, as in their photos.
    "mustang": mustang,
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
        obj = MODELS[name]()
        export(obj, name)
        if name in ITEMS:
            icon(obj, obj, name)


main()
