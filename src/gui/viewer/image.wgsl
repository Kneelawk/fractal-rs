// viewer.wgsl - This file contains the GPU code for rendering an image at a
// specific location.

struct FragmentData {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] texture_position: vec2<f32>;
};

[[block]]
struct Uniforms {
    from_screen: mat4x4<f32>;
    model: mat4x4<f32>;
    to_screen: mat4x4<f32>;
};

[[group(0), binding(0)]]
var<uniform> uniforms: Uniforms;
[[group(0), binding(1)]]
var u_sampler: sampler;
[[group(0), binding(2)]]
var u_texture: texture_2d<f32>;

[[stage(vertex)]]
fn vert_main([[builtin(vertex_index)]] vert_index: u32) -> FragmentData {
    var indexable: array<vec2<f32>,6u> = array<vec2<f32>,6u>(
        vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(-1.0, -1.0)
    );

    var data: FragmentData;
    let xy = indexable[vert_index];
    data.position = uniforms.from_screen * uniforms.model * uniforms.to_screen * vec4<f32>(xy, 0.0, 1.0);
    let small_xy: vec2<f32> = ((xy + vec2<f32>(1.0, 1.0)) / vec2<f32>(2.0));
    data.texture_position = vec2<f32>(small_xy.x, (1.0 - small_xy.y));
    return data;
}

[[stage(fragment)]]
fn frag_main(data: FragmentData) -> [[location(0)]] vec4<f32> {
    return textureSample(u_texture, u_sampler, data.texture_position);
}
