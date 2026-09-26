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
    keeps the model's own flat colours from its picture (a palette); with
    cut, the material keeps its picture's see-through parts (leaf cards)
    and nothing else of the picture."""
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
                if (cut or rgb is None) and image is None:
                    sys.exit(f"ERROR: {file.name}: material {old.name} has no picture to take from")
                made[old.name] = material(key, image if rgb is None else rgb, cutout=image if cut else None)
            slot.material = made[old.name]
    return meshes


def join(parts, name, size=None):
    """join parts into one flat-shaded object named `name`, resting on the
    ground. With `size`, it is scaled evenly so its largest side is that
    many metres (the game's size for it)."""
    bpy.ops.object.select_all(action="DESELECT")
    for p in parts:
        p.select_set(True)
    bpy.context.view_layer.objects.active = parts[0]
    if len(parts) > 1:
        bpy.ops.object.join()
    obj = bpy.context.active_object
    obj.name = name
    bpy.ops.object.shade_flat()
    if size is not None:
        obj.scale *= size / max(obj.dimensions)
        bpy.context.view_layer.update()
    obj.location.z -= min((obj.matrix_world @ v.co).z for v in obj.data.vertices)
    return obj


# -- the picture -------------------------------------------------------------


def picture(obj, out, frame=None):
    """render `obj` (one object or several) from the game's camera to `out`,
    its ground point at the centre. The picture fits the model, or with
    `frame`, is that many metres either side of the ground point (half
    width, half height) as the camera sees it, so pictures that are drawn
    over each other line up."""
    objs = obj if isinstance(obj, list) else [obj]
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
    if frame is None:
        view = cam.matrix_world.inverted()
        seen = [view @ (o.matrix_world @ v.co) for o in objs for v in o.data.vertices]
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


def kenney(kit, model, name, size=ITEM):
    """a model from one of Kenney's kits (scripts/kenney-<kit>, CC0), in
    its own flat colours."""
    return join(load(ROOT / "scripts" / f"kenney-{kit}" / f"{model}.glb", {"colormap": (None, False)}), name, size)


def pipe():
    """the pipe: a metre of rusty steel with a taped grip at one end."""
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
    return join(parts, "pipe", ITEM)


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


def person():
    """the rig standing with its arms at its sides, and its layers: the
    body (with eyes and brows) and one hairstyle, pinned to the head."""
    own = {"MI_": (None, False)}
    meshes = load(CHARACTERS / "Superhero_Male_FullBody.gltf", own)
    # The file carries a loose helper ball; the body is what the rig holds.
    for m in [m for m in meshes if m.parent is None]:
        bpy.data.objects.remove(m, do_unlink=True)
    body = [m for m in meshes if m.parent is not None]
    rig = body[0].parent
    bones = rig.pose.bones
    for b in bones:
        b.rotation_mode = "XYZ"
    # Arms down from the T-pose: each upper arm turns down about its Z.
    bones["upperarm_l"].rotation_euler = (0, 0, math.radians(-72))
    bones["upperarm_r"].rotation_euler = (0, 0, math.radians(72))
    hair = load(CHARACTERS / "Hair_SimpleParted.gltf", own)
    for h in hair:
        world = h.matrix_world.copy()
        h.parent, h.parent_type, h.parent_bone = rig, "BONE", "Head"
        h.matrix_world = world
    bpy.context.view_layer.update()
    return rig, {"body": body, "hair": hair}


def people():
    """render every layer of a person per facing: `person <layer> <facing>`."""
    clear()
    rig, layers = person()
    # Which way the rig faces at rest: from its ankles to its toes.
    bones, m = rig.pose.bones, rig.matrix_world
    toes = (m @ bones["ball_l"].head + m @ bones["ball_r"].head) / 2
    ankles = (m @ bones["foot_l"].head + m @ bones["foot_r"].head) / 2
    rest = math.atan2(toes.y - ankles.y, toes.x - ankles.x)
    for k in range(FACINGS):
        rig.rotation_euler = (0, 0, math.radians(k * 360 / FACINGS) - rest)
        bpy.context.view_layer.update()
        for layer, meshes in layers.items():
            for other, others in layers.items():
                for o in others:
                    o.is_holdout = other != layer
            picture(meshes, OUT / f"person {layer} {k}.png", frame=PERSON_FRAME)


# Models that render several pictures themselves.
SETS = {"person": people}


MODELS = {
    "tree": tree,
    "rock": rock,
    # Items, named as in content/src/lib.rs.
    "note": note,
    "canned food": lambda: kenney("food", "can", "canned food"),
    "water bottle": lambda: kenney("survival", "bottle", "water bottle"),
    "bandage": bandage,
    "storage box": lambda: kenney("survival", "box", "storage box", DEPLOYABLE),
    "scrap": lambda: kenney("survival", "metal-panel-screws-half", "scrap"),
    "cloth": lambda: kenney("survival", "bedroll-packed", "cloth"),
    "pipe": pipe,
    "hatchet": lambda: kenney("survival", "tool-axe", "hatchet"),
    "pipe shotgun": pipe_shotgun,
    "shotgun shell": shotgun_shell,
    "road sign vest": road_sign_vest,
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
