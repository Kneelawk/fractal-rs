let offset: vec2<f32> = vec2<f32>(-0.5, -0.5);

// This constant is designed to have its value replaced.
let radius_squared: f32 = 16.0;

struct FragmentData {
    [[builtin(position)]] position: vec4<f32>;
};

struct View {
    image_size: vec2<f32>;
    image_scale: vec2<f32>;
    plane_start: vec2<f32>;
};

struct FractalOpts {
    c: vec2<f32>;
    iterations: u32;
    mandelbrot: bool;
};

[[block]]
struct Uniforms {
    view: View;
    opts: FractalOpts;
};

var indexable: array<vec2<f32>,6u> = array<vec2<f32>,6u>(
    vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0),
    vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(-1.0, -1.0)
);

[[group(0), binding(0)]]
var<uniform> uniforms: Uniforms;

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
    return dot(a, a);
}

fn linear_intersection(iterations: u32, z_curr: vec2<f32>, z_prev: vec2<f32>) -> f32 {
    let iter = f32(iterations);

    if (length_sqr(z_curr) == length_sqr(z_prev)) {
        return iter;
    }

    if (length_sqr(z_prev) > radius_squared) {
        return iter;
    }

    if (length_sqr(z_curr) < radius_squared) {
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
            f = (m * p + sqrt(radius_squared * m_sqr_1 - p * p)) / m_sqr_1;
        } else {
            f = (m * p - sqrt(radius_squared * m_sqr_1 - p * p)) / m_sqr_1;
        }

        frac = (bx - f) / dx;
    } else {
        let m = dx / dy;
        let m_sqr_1 = m * m + 1.0;
        let p = m * ay - ax;

        var f: f32;
        if (by > ay) {
            f = (m * p + sqrt(radius_squared * m_sqr_1 - p * p)) / m_sqr_1;
        } else {
            f = (m * p - sqrt(radius_squared * m_sqr_1 - p * p)) / m_sqr_1;
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

// This function is designed to have its contents replaced.
fn f(z: vec2<f32>, c: vec2<f32>) -> vec2<f32> {
    return complex_add(complex_sqr(z), c);
}

// This function is designed to have its contents replaced.
fn smooth(iterations: u32, z_curr: vec2<f32>, z_prev: vec2<f32>) -> f32 {
    return f32(iterations);
}

[[stage(fragment)]]
fn frag_main(data: FragmentData) -> [[location(0)]] vec4<f32> {
    // Only generate fractals for the requested area.
    if (data.position.x >= uniforms.view.image_size.x || data.position.y >= uniforms.view.image_size.y) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    let loc = uniforms.view.plane_start + (data.position.xy + offset) * uniforms.view.image_scale;

    var z: vec2<f32>;
    var c: vec2<f32>;

    if (uniforms.opts.mandelbrot) {
        z = vec2<f32>(0.0, 0.0);
        c = loc;
    } else {
        z = loc;
        c = uniforms.opts.c;
    }

    var z_prev: vec2<f32> = z;
    var n: u32 = 0u32;
    for (; n < uniforms.opts.iterations; n = n + 1u32) {
        if (length_sqr(z) > radius_squared) {
            break;
        }

        z_prev = z;
        z = f(z, c);
    }

    if (n >= uniforms.opts.iterations) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    } else {
        let v = smooth(n, z, z_prev);
        return fromHSB((v * 3.3 / 256.0) % 1.0, 1.0, (v / 16.0) % 1.0, 1.0);
    }
}
