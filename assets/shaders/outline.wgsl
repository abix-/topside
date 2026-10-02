// Outlines (src/outline.rs): a dark line where the depth bends or the way
// a surface faces turns. Bevy's depth is reverse z (1 / distance), which
// runs straight across a flat surface on screen, so flat floors and walls,
// tile beside tile, make no line; a corner, an edge, or something in front
// of something else does.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var depth: texture_depth_2d;
@group(0) @binding(3) var normals: texture_2d<f32>;

// The line's colour.
const LINE: vec3<f32> = vec3<f32>(0.04, 0.04, 0.05);
// How far the depth may bend from straight, against the depth here.
const DEPTH_BEND: f32 = 0.03;
// How far two neighbouring surfaces may face apart (1 - their cosine).
const FACING_TURN: f32 = 0.25;
// How many pixels apart the neighbours are, the line's half width: up
// close, and past THIN_PAST metres. A line stays the same pixels however
// far away, so far things would drown in their lines: they thin, then
// fade from FADE_FROM metres to nothing at FADE_TO (operator, 2026-10-01).
const WIDTH: i32 = 2;
const WIDTH_FAR: i32 = 1;
const THIN_PAST: f32 = 15.0;
const FADE_FROM: f32 = 25.0;
const FADE_TO: f32 = 60.0;
// The camera's near plane (Bevy's default; topside sets none): depth is
// NEAR over the distance.
const NEAR: f32 = 0.1;

fn depth_at(p: vec2<i32>) -> f32 {
    let size = vec2<i32>(textureDimensions(depth));
    return textureLoad(depth, clamp(p, vec2<i32>(0), size - 1), 0);
}

fn normal_at(p: vec2<i32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(normals));
    return textureLoad(normals, clamp(p, vec2<i32>(0), size - 1), 0);
}

fn facing_at(p: vec2<i32>) -> vec3<f32> {
    return normal_at(p).xyz * 2.0 - 1.0;
}

// Whether what is drawn here gets lines: the normal's alpha, written by
// the cel material's prepass (0 for a material cut out by its alpha:
// leaves).
fn outlined_at(p: vec2<i32>) -> bool {
    return normal_at(p).a > 0.5;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen, screen_sampler, in.uv);
    let p = vec2<i32>(in.position.xy);
    let d = depth_at(p);
    // Nothing drawn here gets lines: no line.
    if d > 0.0 && !outlined_at(p) {
        return color;
    }
    // How far away this pixel is, in metres (the sky: as far as can be).
    let far = NEAR / max(d, 1e-6);
    if far >= FADE_TO {
        return color;
    }
    let w = select(WIDTH, WIDTH_FAR, far > THIN_PAST);
    let offsets = array<vec2<i32>, 4>(vec2<i32>(w, 0), vec2<i32>(-w, 0), vec2<i32>(0, w), vec2<i32>(0, -w));
    let strength = 1.0 - smoothstep(FADE_FROM, FADE_TO, far);
    var sum = 0.0;
    var nearest = d;
    for (var i = 0; i < 4; i++) {
        var n = depth_at(p + offsets[i]);
        // A neighbour with no lines in front of this pixel (a leaf) counts
        // as this pixel; behind it, it still makes this pixel's edge.
        if n > d && !outlined_at(p + offsets[i]) {
            n = d;
        }
        sum += n;
        nearest = max(nearest, n);
    }
    // The bend: the neighbours' depth against straight through this pixel.
    let bend = abs(sum - 4.0 * d) / max(nearest, 1e-6);
    var turn = 0.0;
    if d > 0.0 {
        let here = facing_at(p);
        for (var i = 0; i < 4; i++) {
            if depth_at(p + offsets[i]) > 0.0 && outlined_at(p + offsets[i]) {
                turn = max(turn, 1.0 - dot(here, facing_at(p + offsets[i])));
            }
        }
    }
    let edge = select(0.0, 1.0, bend > DEPTH_BEND || turn > FACING_TURN);
    return vec4<f32>(mix(color.rgb, LINE, edge * strength), color.a);
}
