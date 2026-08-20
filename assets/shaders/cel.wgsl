#import bevy_pbr::{
    mesh_functions,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions,
}

struct CelUniforms {
    color: vec4<f32>,
};

@group(2) @binding(0)
var<uniform> material: CelUniforms;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    var out: FragmentOutput;

    let normal = normalize(in.world_normal);
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let ndotl = dot(normal, light_dir);

    // three bands: shadow, mid, lit
    var band: f32;
    if ndotl < 0.0 {
        band = 0.4;
    } else if ndotl < 0.5 {
        band = 0.7;
    } else {
        band = 1.0;
    }

    out.color = vec4<f32>(material.color.rgb * band, material.color.a);
    return out;
}
