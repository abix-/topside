// The cel shade (design.md: a base colour cut into hard light bands), on
// top of Bevy's standard material (its colour, its palette picture, its
// vertex colours, its see-through cut-outs): the light from the sun is cut
// into three hard steps, no blending, no shine.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    forward_io::{VertexOutput, FragmentOutput},
}

// Towards the sun, in the world (src/model.rs CelShade::sun).
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> sun: vec4<f32>;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    let light = dot(normalize(pbr_input.world_normal), normalize(sun.xyz));
    // The bands: (light above this, brightness).
    var band = 0.45;
    if light > 0.6 {
        band = 1.0;
    } else if light > 0.25 {
        band = 0.75;
    }
    var out: FragmentOutput;
    out.color = vec4<f32>(color.rgb * band, color.a);
    return out;
}
