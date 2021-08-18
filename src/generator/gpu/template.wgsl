struct FragmentData {
    [[builtin(position)]] position: vec4<f32>;
};

struct View {
    image_size: vec2<f32>;
    image_scale: vec2<f32>;
    plane_start: vec2<f32>;
};

[[block]]
struct Uniforms {
    view: View;
};

var indexable: array<vec2<f32>,6u> = array<vec2<f32>,6u>(
    vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0),
    vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(-1.0, -1.0)
);

[[group(0), binding(0)]]
var<uniform> uniforms: Uniforms;

let offset: vec2<f32> = vec2<f32>(-0.5, -0.5);
let iterations: i32 = 200;
let c: vec2<f32> = vec2<f32>(0.16611, 0.59419);

[[stage(vertex)]]
fn vert_main([[builtin(vertex_index)]] vert_index: u32) -> FragmentData {
    var data: FragmentData;
    let xy = indexable[vert_index];
    data.position = vec4<f32>(xy, 0.0, 1.0);
    return data;
}

fn complex_add(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return a + b;
}

fn complex_multiply(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

fn complex_sqr(a: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * a.x - a.y * a.y, 2.0 * a.x * a.y);
}

fn length_sqr(a: vec2<f32>) -> f32 {
    return a.x * a.x + a.y * a.y;
}

fn fromHSB(hue: f32, saturation: f32, brightness: f32, alpha: f32) -> vec4<f32> {
    if (saturation == 0.0) {
        return vec4<f32>(brightness, brightness, brightness, alpha);
    } else {
        let sector = (hue % 1.0) * 6.0;
        let offset = sector - floor(sector);
        let off = brightness * (1.0 - saturation);
        let fadeOut = brightness * (1.0 - (saturation * offset));
        let fadeIn = brightness * (1.0 - (saturation * (1.0 - offset)));
        switch(i32(sector)) {
            case 0: {
                return vec4<f32>(brightness, fadeIn, off, alpha);
            }
            case 1: {
                return vec4<f32>(fadeOut, brightness, off, alpha);
            }
            case 2: {
                return vec4<f32>(off, brightness, fadeIn, alpha);
            }
            case 3: {
                return vec4<f32>(off, fadeOut, brightness, alpha);
            }
            case 4: {
                return vec4<f32>(fadeIn, off, brightness, alpha);
            }
            case 5: {
                return vec4<f32>(brightness, off, fadeOut, alpha);
            }
            default: {
                return vec4<f32>(0.0, 0.0, 0.0, alpha);
            }
        }
    }
}

fn f(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    return complex_add(complex_sqr(z), c);
}

[[stage(fragment)]]
fn frag_main(data: FragmentData) -> [[location(0)]] vec4<f32> {
    // Only generate fractals for the requested area.
    if (data.position.x >= uniforms.view.image_size.x || data.position.y >= uniforms.view.image_size.y) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    var z = uniforms.view.plane_start + (data.position.xy + offset) * uniforms.view.image_scale;

    var n: i32 = 0;
    for (; n < iterations; n = n + 1) {
        if (length_sqr(z) > 16.0) {
            break;
        }

        z = f(z, c);
    }

    if (n >= iterations) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    } else {
        let v = f32(n);
        return fromHSB((v * 3.3 / 256.0) % 1.0, 1.0, (v / 16.0) % 1.0, 1.0);
    }
}
