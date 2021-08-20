// template.wgsl - This file describes the general process for generating
// fractals using WGPU. This file is a template. Key constants and functions are
// replaced when this file is loaded, allowing efficient manipulation of the
// fractal generator.

//
// Structs
//

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

//
// Constants
//

let offset: vec2<f32> = vec2<f32>(-0.5, -0.5);

var indexable: array<vec2<f32>,6u> = array<vec2<f32>,6u>(
    vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0),
    vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(-1.0, -1.0)
);

//
// Template Constants
//

// This constant is designed to have its value replaced.
let t_c_real: f32 = 0.0;

// This constant is designed to have its value replaced.
let t_c_imag: f32 = 0.0;

// This constant is designed to have its value replaced.
let t_iterations: u32 = 0u32;

// This constant is designed to have its value replaced.
let t_mandelbrot: bool = false;

// This constant is designed to have its value replaced.
let t_radius_squared: f32 = 0.0;

// This constant is designed to have its value replaced.
let t_sample_count: u32 = 1u32;

//
// Uniforms
//

[[group(0), binding(0)]]
var<uniform> uniforms: Uniforms;

//
// Vertex Shader
//

[[stage(vertex)]]
fn vert_main([[builtin(vertex_index)]] vert_index: u32) -> FragmentData {
    var data: FragmentData;
    let xy = indexable[vert_index];
    data.position = vec4<f32>(xy, 0.0, 1.0);
    return data;
}

//
// Utility Functions
//

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
    return dot(a, a);
}

fn linear_intersection(iterations: u32, z_curr: vec2<f32>, z_prev: vec2<f32>) -> f32 {
    let iter = f32(iterations);

    if (length_sqr(z_curr) == length_sqr(z_prev)) {
        return iter;
    }

    if (length_sqr(z_prev) > t_radius_squared) {
        return iter;
    }

    if (length_sqr(z_curr) < t_radius_squared) {
        return iter;
    }

    let ax = z_prev.x;
    let ay = z_prev.y;
    let bx = z_curr.x;
    let by = z_curr.y;
    let dx = bx - ax;
    let dy = by - ay;

    var frac: f32;
    if (abs(dx) > abs(dy)) {
        let m = dy / dx;
        let m_sqr_1 = m * m + 1.0;
        let p = m * ax - ay;

        var f: f32;
        if (bx > ax) {
            f = (m * p + sqrt(t_radius_squared * m_sqr_1 - p * p)) / m_sqr_1;
        } else {
            f = (m * p - sqrt(t_radius_squared * m_sqr_1 - p * p)) / m_sqr_1;
        }

        frac = (bx - f) / dx;
    } else {
        let m = dx / dy;
        let m_sqr_1 = m * m + 1.0;
        let p = m * ay - ax;

        var f: f32;
        if (by > ay) {
            f = (m * p + sqrt(t_radius_squared * m_sqr_1 - p * p)) / m_sqr_1;
        } else {
            f = (m * p - sqrt(t_radius_squared * m_sqr_1 - p * p)) / m_sqr_1;
        }

        frac = (by - f) / dy;
    }

    return iter - frac;
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

//
// Template Functions
//

// This function is designed to have its contents replaced.
fn t_sample_offsets() -> array<vec2<f32>, t_sample_count> {
    return array<vec2<f32>, t_sample_count>(vec2<f32>(0.0, 0.0));
}

// This function is designed to have its contents replaced.
fn t_f(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    return complex_add(complex_sqr(z), c);
}

// This function is designed to have its contents replaced.
fn t_smooth(iterations: u32, z_curr: vec2<f32>, z_prev: vec2<f32>) -> f32 {
    return f32(iterations);
}

//
// Generator Functions
//

fn gen_pixel(pixel_location: vec2<f32>) -> vec4<f32> {
    let loc = uniforms.view.plane_start + (pixel_location + offset) * uniforms.view.image_scale;

    var z: vec2<f32>;
    var c: vec2<f32>;

    if (t_mandelbrot) {
        z = vec2<f32>(0.0, 0.0);
        c = loc;
    } else {
        z = loc;
        c = vec2<f32>(t_c_real, t_c_imag);
    }

    var z_prev: vec2<f32> = z;
    var n: u32 = 0u32;
    for (; n < t_iterations; n = n + 1u32) {
        if (length_sqr(z) > t_radius_squared) {
            break;
        }

        z_prev = z;
        z = t_f(z, c);
    }

    if (n >= t_iterations) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    } else {
        let v = t_smooth(n, z, z_prev);
        return fromHSB((v * 3.3 / 256.0) % 1.0, 1.0, (v / 16.0) % 1.0, 1.0);
    }
}

[[stage(fragment)]]
fn frag_main(data: FragmentData) -> [[location(0)]] vec4<f32> {
    // Only generate fractals for the requested area.
    if (data.position.x >= uniforms.view.image_size.x || data.position.y >= uniforms.view.image_size.y) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    let sample_count_f32 = f32(t_sample_count);
    var sample_offsets = t_sample_offsets();

    var color = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    for (var i = 0u32; i < t_sample_count; i = i + 1u32) {
        color = color + gen_pixel(data.position.xy + sample_offsets[i]) / sample_count_f32;
    }

    return color;
}
