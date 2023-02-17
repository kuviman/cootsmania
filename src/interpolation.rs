use super::*;

pub struct Interpolated<T> {
    a: T,
    b: T,
    c: T,
    d: T,
    t: f32,
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

const INTERPOLATION_TIME: f32 = 0.2;

impl<T: Mul<f32, Output = T> + Add<Output = T> + Sub<Output = T> + Copy + Zero> Interpolated<T> {
    pub fn new(p: T, v: T) -> Self {
        Self {
            a: T::ZERO,
            b: T::ZERO,
            c: v * INTERPOLATION_TIME,
            d: p,
            t: 0.0,
        }
    }
    pub fn teleport(&mut self, p: T, v: T) {
        self.a = T::ZERO;
        self.b = T::ZERO;
        self.c = v * INTERPOLATION_TIME;
        self.d = p;
        self.t = 0.0;
    }
    pub fn server_update(&mut self, p2: T, v2: T) {
        let p2 = p2 + v2 * INTERPOLATION_TIME;
        let p1 = self.get();
        let v1 = self.get_derivative();
        self.d = p1;
        self.c = v1 * INTERPOLATION_TIME;
        self.b = p2 * 3.0 - self.c * 2.0 - self.d * 3.0 - v2 * INTERPOLATION_TIME;
        self.a = p2 - self.b - self.c - self.d;
        self.t = 0.0;
    }
    pub fn update(&mut self, delta_time: f32) {
        self.t = (self.t + delta_time / INTERPOLATION_TIME).min(1.0);
    }
    pub fn get(&self) -> T {
        self.a * self.t.powi(3) + self.b * self.t.sqr() + self.c * self.t + self.d
    }
    pub fn get_derivative(&self) -> T {
        (self.a * 3.0 * self.t.sqr() + self.b * 2.0 * self.t + self.c) * (1.0 / INTERPOLATION_TIME)
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
    i.update(INTERPOLATION_TIME);
    assert!(i.get() == 1.0);
    assert!(i.get_derivative() == 1.0);
    i.update(-INTERPOLATION_TIME / 2.0);
    assert!(i.get() == 0.5);
    assert!(i.get_derivative() == 1.0);
}
