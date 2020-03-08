/// Computes the modulus of a float between two values accounting for negatives.
fn mod2(mut value: f32, min: f32, max: f32) -> f32 {
    let size = max - min;

    value = (value - min) % size;

    if value >= 0f32 {
        value + min
    } else {
        size + value + min
    }
}