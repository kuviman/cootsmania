use super::*;

pub struct Interpolated<T> {
    a: T,
    b: T,
    c: T,
    d: T,
    t: f32,
    interpolation_time: f32,
}

// f(t) = A * t^3 + B * t^2 + C * t + D
// f(0) = p1
// f'(0) = v1 * IT
// f(1) = p2
// f'(1) = v2 * IT

// D = p1
// C = v1 * IT
// A + B + C + D = p2
// 3A + 2B + C = v2 * IT

// A = p2 - B - C - D
// 3 (p2 - B - C - D) + 2B + C = v2 * IT
// 3 p2 - 3B - 3C - 3D + 2B + C = v2 * IT
// B = 3p2 - 2C - 3D - v2 * IT

pub trait Zero {
    const ZERO: Self;
}

impl Zero for f32 {
    const ZERO: Self = 0.0;
}

impl Zero for vec2<f32> {
    const ZERO: Self = vec2::ZERO;
}

impl Zero for vec3<f32> {
    const ZERO: Self = vec3::ZERO;
}

const MIN_ITERPOLATION_TIME: f32 = 0.05;

impl<T: Mul<f32, Output = T> + Add<Output = T> + Sub<Output = T> + Copy + Zero> Interpolated<T> {
    pub fn new(p: T, v: T) -> Self {
        let interpolation_time = MIN_ITERPOLATION_TIME;
        Self {
            a: T::ZERO,
            b: T::ZERO,
            c: v * interpolation_time,
            d: p,
            t: 0.0,
            interpolation_time,
        }
    }
    pub fn teleport(&mut self, p: T, v: T) {
        self.a = T::ZERO;
        self.b = T::ZERO;
        self.c = v * self.interpolation_time;
        self.d = p;
        self.t = 0.0;
    }
    pub fn server_update(&mut self, p2: T, v2: T) {
        let p1 = self.get();
        let v1 = self.get_derivative();
        let interpolation_time = (self.t * 1.5).max(MIN_ITERPOLATION_TIME);
        // let p2 = p2 + v2 * interpolation_time; // Prediction
        let d = p1;
        let c = v1 * interpolation_time;
        let b = p2 * 3.0 - c * 2.0 - d * 3.0 - v2 * interpolation_time;
        let a = p2 - b - c - d;
        *self = Self {
            a,
            b,
            c,
            d,
            t: 0.0,
            interpolation_time,
        };
    }
    pub fn update(&mut self, delta_time: f32) {
        self.t += delta_time;
    }
    pub fn get(&self) -> T {
        let t = (self.t / self.interpolation_time).min(1.0);
        self.a * t.powi(3) + self.b * t.sqr() + self.c * t + self.d
    }
    pub fn get_derivative(&self) -> T {
        let t = (self.t / self.interpolation_time).min(1.0);
        (self.a * 3.0 * t.sqr() + self.b * 2.0 * t + self.c) * (1.0 / self.interpolation_time)
    }
}

#[test]
fn test_interpolation() {
    let mut i = Interpolated::new(0.0, 1.0);
    assert!(i.get() == 0.0);
    assert!(i.get_derivative() == 1.0);
    i.server_update(1.0, 1.0);
    assert!(i.get() == 0.0);
    assert!(i.get_derivative() == 1.0);
    i.update(MIN_INTERPOLATION_TIME);
    assert!(i.get() == 1.0);
    assert!(i.get_derivative() == 1.0);
    i.update(-MIN_INTERPOLATION_TIME / 2.0);
    assert!(i.get() == 0.5);
    assert!(i.get_derivative() == 1.0);
}
