#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material_color: vec4<f32>;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(mesh.world_normal);
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let ndotl = dot(normal, light_dir);

    var band: f32;
    if ndotl < 0.0 {
        band = 0.4;
    } else if ndotl < 0.5 {
        band = 0.7;
    } else {
        band = 1.0;
    }

    return vec4<f32>(material_color.rgb * band, material_color.a);
}
