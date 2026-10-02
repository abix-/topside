// The cel material's prepass (src/model.rs CelShade): Bevy's own
// standard material prepass (bevy_pbr render/pbr_prepass.wgsl) for what
// topside's models use (no normal maps, no meshlets, not bindless), with
// one thing more: whether the outline pass (src/outline.rs) draws lines
// here, in the normal's alpha (1 outlined, 0 not: a material cut out by
// its alpha, the leaves).

#import bevy_pbr::{
    pbr_prepass_functions,
    pbr_bindings,
    pbr_types,
    pbr_functions,
    prepass_io,
}

struct CelShade {
    sun: vec4<f32>,
    outlined: f32,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> cel: CelShade;

#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(in: prepass_io::VertexOutput, @builtin(front_facing) is_front: bool) -> prepass_io::FragmentOutput {
    pbr_prepass_functions::prepass_alpha_discard(in);
    var out: prepass_io::FragmentOutput;

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif

#ifdef NORMAL_PREPASS
    let flags = pbr_bindings::material.flags;
    var normal = in.world_normal;
    if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        let double_sided = (flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
        normal = pbr_functions::prepare_world_normal(in.world_normal, double_sided, is_front);
    }
    out.normal = vec4(normal * 0.5 + vec3(0.5), cel.outlined);
#endif

#ifdef MOTION_VECTOR_PREPASS
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);
#endif

    return out;
}
#endif
