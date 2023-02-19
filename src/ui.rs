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
