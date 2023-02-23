use super::*;

pub struct TextureButton<'a> {
    sense: &'a mut geng::ui::Sense,
    clicked: bool,
    texture: &'a ugli::Texture,
    size: f64,
}

impl<'a> TextureButton<'a> {
    pub fn new(cx: &'a geng::ui::Controller, texture: &'a ugli::Texture, size: f64) -> Self {
        let sense: &'a mut geng::ui::Sense = cx.get_state();
        Self {
            clicked: sense.take_clicked(),
            sense,
            texture,
            size,
        }
    }
    pub fn was_clicked(&self) -> bool {
        self.clicked
    }
}

impl geng::ui::Widget for TextureButton<'_> {
    fn sense(&mut self) -> Option<&mut geng::ui::Sense> {
        Some(self.sense)
    }
    fn calc_constraints(&mut self, _cx: &geng::ui::ConstraintsContext) -> geng::ui::Constraints {
        geng::ui::Constraints {
            min_size: vec2(self.size, self.size),
            flex: vec2::ZERO,
        }
    }
    fn draw(&mut self, cx: &mut geng::ui::DrawContext) {
        let extra = 0.2;
        let size = if self.sense.is_captured() {
            1.0 - extra
        } else if self.sense.is_hovered() {
            1.0 + extra
        } else {
            1.0
        };
        cx.geng.draw_2d(
            cx.framebuffer,
            &geng::PixelPerfectCamera,
            &draw_2d::TexturedQuad::unit(self.texture)
                .scale_uniform(size)
                .scale(cx.position.size().map(|x| x as f32 / 2.0))
                .translate(cx.position.center().map(|x| x as f32)),
        );
    }
}

pub struct TextureWidget<'a> {
    texture: &'a ugli::Texture,
    size: f64,
}

impl<'a> TextureWidget<'a> {
    pub fn new(texture: &'a ugli::Texture, size: f64) -> Self {
        Self { texture, size }
    }
}

impl geng::ui::Widget for TextureWidget<'_> {
    fn calc_constraints(&mut self, _cx: &geng::ui::ConstraintsContext) -> geng::ui::Constraints {
        geng::ui::Constraints {
            min_size: vec2(self.size, self.size),
            flex: vec2::ZERO,
        }
    }
    fn draw(&mut self, cx: &mut geng::ui::DrawContext) {
        cx.geng.draw_2d(
            cx.framebuffer,
            &geng::PixelPerfectCamera,
            &draw_2d::TexturedQuad::new(cx.position.map(|x| x as f32), self.texture),
        );
    }
}

pub struct CarWidget<'a> {
    texture: &'a ugli::Texture,
    color_texture: &'a ugli::Texture,
    color: Rgba<f32>,
    size: f64,
}

impl<'a> CarWidget<'a> {
    pub fn new(
        texture: &'a ugli::Texture,
        color_texture: &'a ugli::Texture,
        color: Rgba<f32>,
        size: f64,
    ) -> Self {
        Self {
            texture,
            color_texture,
            color,
            size,
        }
    }
}

impl geng::ui::Widget for CarWidget<'_> {
    fn calc_constraints(&mut self, _cx: &geng::ui::ConstraintsContext) -> geng::ui::Constraints {
        geng::ui::Constraints {
            min_size: vec2(self.size, self.size),
            flex: vec2::ZERO,
        }
    }
    fn draw(&mut self, cx: &mut geng::ui::DrawContext) {
        cx.geng.draw_2d(
            cx.framebuffer,
            &geng::PixelPerfectCamera,
            &draw_2d::TexturedQuad::unit(self.texture)
                .rotate(-f32::PI / 3.0)
                .scale(vec2(1.0, 0.65)) // HARDCODE LUL
                .translate(vec2(0.0, -0.5))
                .scale_uniform(cx.position.width() as f32 / 2.0)
                .translate(cx.position.center().map(|x| x as f32)),
        );
        cx.geng.draw_2d(
            cx.framebuffer,
            &geng::PixelPerfectCamera,
            &draw_2d::TexturedQuad::unit_colored(self.color_texture, self.color)
                .rotate(-f32::PI / 3.0)
                .scale(vec2(1.0, 0.65)) // HARDCODE LUL
                .translate(vec2(0.0, -0.5))
                .scale_uniform(cx.position.width() as f32 / 2.0)
                .translate(cx.position.center().map(|x| x as f32)),
        );
    }
}

pub struct CustomSlider<'a> {
    sense: &'a mut geng::ui::Sense,
    pos: &'a mut Option<Aabb2<f64>>,
    tick_radius: &'a mut f32,
    value: f64,
    range: RangeInclusive<f64>,
    line_texture: &'a ugli::Texture,
    knob_texture: &'a ugli::Texture,
    f: Box<dyn FnMut(f64) + 'a>,
}

impl<'a> CustomSlider<'a> {
    const ANIMATION_SPEED: f32 = 5.0;

    pub fn new(
        cx: &'a geng::ui::Controller,
        line_texture: &'a ugli::Texture,
        knob_texture: &'a ugli::Texture,
        value: f64,
        range: RangeInclusive<f64>,
        f: Box<dyn FnMut(f64) + 'a>,
    ) -> Self {
        CustomSlider {
            sense: cx.get_state(),
            line_texture,
            knob_texture,
            tick_radius: cx.get_state(),
            pos: cx.get_state(),
            value,
            range,
            f,
        }
    }
}

impl<'a> geng::ui::Widget for CustomSlider<'a> {
    fn sense(&mut self) -> Option<&mut geng::ui::Sense> {
        Some(self.sense)
    }
    fn update(&mut self, delta_time: f64) {
        let target_tick_radius = if self.sense.is_hovered() || self.sense.is_captured() {
            1.0 / 2.0
        } else {
            1.0 / 6.0
        };
        *self.tick_radius += (target_tick_radius - *self.tick_radius)
            .clamp_abs(Self::ANIMATION_SPEED * delta_time as f32);
    }
    fn draw(&mut self, cx: &mut geng::ui::DrawContext) {
        *self.pos = Some(cx.position);
        let geng = cx.geng;
        geng.draw_2d(
            cx.framebuffer,
            &geng::PixelPerfectCamera,
            &draw_2d::TexturedQuad::new(cx.position.map(|x| x as f32), self.line_texture),
        );
        let extra = 0.2;
        let size = if self.sense.is_captured() {
            1.0 - extra
        } else if self.sense.is_hovered() {
            1.0 + extra
        } else {
            1.0
        };
        let knob_radius = cx.position.height() / 2.0;
        geng.draw_2d(
            cx.framebuffer,
            &geng::PixelPerfectCamera,
            &draw_2d::TexturedQuad::unit(self.knob_texture)
                .scale_uniform(knob_radius as f32 * size)
                .translate(
                    vec2(
                        cx.position.min.x
                            + knob_radius
                            + (cx.position.width() - 2.0 * knob_radius) * self.value
                                / (*self.range.end() - *self.range.start()),
                        cx.position.center().y,
                    )
                    .map(|x| x as f32),
                ),
        );
    }
    fn handle_event(&mut self, event: &geng::Event) {
        let aabb = match *self.pos {
            Some(pos) => pos,
            None => return,
        };
        if self.sense.is_captured() {
            if let geng::Event::MouseDown { position, .. }
            | geng::Event::MouseMove { position, .. } = &event
            {
                let position = position.x - aabb.min.x;
                let knob_size = aabb.height() / 2.0;
                let t = (position - knob_size) / (aabb.width() - 2.0 * knob_size);
                let new_value = *self.range.start()
                    + t.clamp(0.0, 1.0) * (*self.range.end() - *self.range.start());
                (self.f)(new_value);
            }
        }
    }

    fn calc_constraints(
        &mut self,
        _children: &geng::ui::ConstraintsContext,
    ) -> geng::ui::Constraints {
        geng::ui::Constraints::default()
    }
}
