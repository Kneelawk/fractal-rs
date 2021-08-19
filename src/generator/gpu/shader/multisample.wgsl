// multisample.wgsl - This shader is responsible for taking the textures
// generated by the fractal generator and blending them together.

struct FragmentData {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] texture_position: vec2<f32>;
};

var indexable: array<vec2<f32>,6u> = array<vec2<f32>,6u>(
    vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0),
    vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(-1.0, -1.0)
);

[[group(0), binding(0)]]
var s: sampler;

[[group(0), binding(1)]]
var t1: texture_2d<f32>;

[[group(0), binding(2)]]
var t2: texture_2d<f32>;

[[group(0), binding(3)]]
var t3: texture_2d<f32>;

[[group(0), binding(4)]]
var t4: texture_2d<f32>;

[[stage(vertex)]]
fn vert_main([[builtin(vertex_index)]] vert_index: u32) -> FragmentData {
    var data: FragmentData;
    let xy = indexable[vert_index];
    data.position = vec4<f32>(xy, 0.0, 1.0);
    let small_xy = (xy + vec2<f32>(1.0, 1.0)) / 2.0;
    data.texture_position = vec2<f32>(small_xy.x, 1.0 - small_xy.y);
    return data;
}

[[stage(fragment)]]
fn frag_main(data: FragmentData) -> [[location(0)]] vec4<f32> {
    let color1 = textureSample(t1, s, data.texture_position);
    let color2 = textureSample(t2, s, data.texture_position);
    let color3 = textureSample(t3, s, data.texture_position);
    let color4 = textureSample(t4, s, data.texture_position);

    let color = (color1 + color2 + color3 + color4) / 4.0;
    return color;
}
