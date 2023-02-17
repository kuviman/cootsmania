use geng::prelude::*;

#[derive(geng::Assets, Serialize, Deserialize)]
#[asset(json)]
pub struct Level {
    segments: Vec<[vec2<f32>; 2]>,
}

const SNAP_DISTANCE: f32 = 0.5;

fn vector_from(p: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>) -> vec2<f32> {
    if vec2::dot(p - p1, p2 - p1) < 0.0 {
        return p1 - p;
    }
    if vec2::dot(p - p2, p1 - p2) < 0.0 {
        return p2 - p;
    }
    let n = (p2 - p1).rotate_90();
    // dot(p + n * t - p1, n) = 0
    // dot(p - p1, n) + dot(n, n) * t = 0
    let t = vec2::dot(p1 - p, n) / vec2::dot(n, n);
    n * t
}

impl Level {
    pub fn save(&self, path: impl AsRef<std::path::Path>) {
        let file = std::fs::File::create(path).expect("Failed to create level file");
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self).expect("Failed to serialize level")
    }
    pub fn snap(&self, pos: vec2<f32>) -> vec2<f32> {
        self.segments
            .iter()
            .copied()
            .flatten()
            .filter(|&p| (pos - p).len() < SNAP_DISTANCE)
            .min_by_key(|&p| r32((pos - p).len()))
            .unwrap_or(pos)
    }
    pub fn hovered_segment(&self, pos: vec2<f32>) -> Option<usize> {
        self.segments
            .iter()
            .position(|&[p1, p2]| vector_from(pos, p1, p2).len() < SNAP_DISTANCE)
    }
}

#[derive(geng::Assets)]
pub struct Assets {}

#[derive(geng::Assets, Deserialize)]
#[asset(json)]
pub struct Config {
    pub rotation_speed: f32,
    pub acceleration: f32,
    pub backward_acceleration: f32,
    pub deceleration: f32,
    pub drift_deceleration: f32,
    pub player_radius: f32,
    pub max_speed: f32,
    pub max_backward_speed: f32,
    pub collision_bounciness: f32,
}

pub struct Player {
    pub pos: vec2<f32>,
    pub vel: vec2<f32>,
    pub rot: f32,
}

pub struct PlayerInput {
    rotate: f32,     // -1 .. 1
    accelerate: f32, // -1 .. 1
}

pub struct Game {
    geng: Geng,
    assets: Rc<Assets>,
    config: Rc<Config>,
    player: Player,
    camera: geng::Camera2d,
    level: Level,
    args: Args,
    start_drag: Option<vec2<f32>>,
    framebuffer_size: vec2<f32>,
}

impl Game {
    pub fn new(
        geng: &Geng,
        assets: &Rc<Assets>,
        level: Level,
        config: &Rc<Config>,
        args: Args,
    ) -> Self {
        Self {
            geng: geng.clone(),
            assets: assets.clone(),
            level,
            config: config.clone(),
            player: Player {
                pos: vec2::ZERO,
                vel: vec2::ZERO,
                rot: 0.0,
            },
            camera: geng::Camera2d {
                center: vec2::ZERO,
                rotation: 0.0,
                fov: 50.0,
            },
            args,
            start_drag: None,
            framebuffer_size: vec2(1.0, 1.0),
        }
    }
}

impl geng::State for Game {
    fn update(&mut self, delta_time: f64) {
        let delta_time = delta_time as f32;

        let input = PlayerInput {
            rotate: {
                let mut value: f32 = 0.0;
                if self.geng.window().is_key_pressed(geng::Key::Left) {
                    value += 1.0;
                }
                if self.geng.window().is_key_pressed(geng::Key::Right) {
                    value -= 1.0;
                }
                value.clamp(-1.0, 1.0)
            },
            accelerate: {
                let mut value: f32 = 0.0;
                if self.geng.window().is_key_pressed(geng::Key::Down) {
                    value -= 1.0;
                }
                if self.geng.window().is_key_pressed(geng::Key::Up) {
                    value += 1.0;
                }
                value.clamp(-1.0, 1.0)
            },
        };

        let player = &mut self.player;
        player.rot += input.rotate * self.config.rotation_speed * delta_time;
        let dir = vec2(1.0, 0.0).rotate(player.rot);

        let mut forward_vel = vec2::dot(dir, player.vel);
        let forward_acceleration = if input.accelerate > 0.0 {
            let target_forward_vel = input.accelerate * self.config.max_speed;
            if target_forward_vel > forward_vel {
                if forward_vel < 0.0 {
                    self.config.deceleration
                } else {
                    self.config.acceleration
                }
            } else {
                -self.config.deceleration
            }
        } else {
            let target_forward_vel = input.accelerate * self.config.max_backward_speed;
            if target_forward_vel < forward_vel {
                if forward_vel > 0.0 {
                    -self.config.deceleration
                } else {
                    -self.config.backward_acceleration
                }
            } else {
                self.config.deceleration
            }
        };
        forward_vel += forward_acceleration * delta_time;

        let mut drift_vel = vec2::skew(dir, player.vel);
        drift_vel -= drift_vel.signum() * self.config.drift_deceleration * delta_time;

        player.vel = dir * forward_vel + dir.rotate_90() * drift_vel;

        player.pos += player.vel * delta_time;
        for &[p1, p2] in &self.level.segments {
            let v = -vector_from(player.pos, p1, p2);
            let penetration = self.config.player_radius - v.len();
            let n = v.normalize_or_zero();
            if penetration > 0.0 {
                player.pos += n * penetration;
                player.vel -=
                    n * vec2::dot(n, player.vel) * (1.0 + self.config.collision_bounciness);
            }
        }
    }
    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        self.framebuffer_size = framebuffer.size().map(|x| x as f32);
        ugli::clear(framebuffer, Some(Rgba::BLACK), None, None);

        let camera = &self.camera;
        let player = &self.player;
        self.geng.draw_2d(
            framebuffer,
            camera,
            &draw_2d::Ellipse::circle(player.pos, self.config.player_radius, Rgba::WHITE),
        );
        self.geng.draw_2d(
            framebuffer,
            camera,
            &draw_2d::Ellipse::circle(
                player.pos
                    + vec2(1.0, 0.0).rotate(player.rot) * self.config.player_radius * (1.0 - 0.1),
                self.config.player_radius * 0.1,
                Rgba::BLACK,
            ),
        );
        for &[p1, p2] in &self.level.segments {
            self.geng.draw_2d(
                framebuffer,
                camera,
                &draw_2d::Segment::new(Segment(p1, p2), 0.1, Rgba::WHITE),
            );
        }

        let cursor_pos = self.camera.screen_to_world(
            self.framebuffer_size,
            self.geng.window().mouse_pos().map(|x| x as f32),
        );
        let snapped_cursor_pos = self.level.snap(cursor_pos);
        if let Some(start) = self.start_drag {
            let end = snapped_cursor_pos;
            self.geng.draw_2d(
                framebuffer,
                camera,
                &draw_2d::Segment::new(Segment(start, end), 0.1, Rgba::GRAY),
            );
        }
        if self.args.editor {
            self.geng.draw_2d(
                framebuffer,
                camera,
                &draw_2d::Quad::new(
                    Aabb2::point(snapped_cursor_pos).extend_uniform(0.3),
                    Rgba::RED,
                ),
            );
            if let Some(index) = self.level.hovered_segment(cursor_pos) {
                let [p1, p2] = self.level.segments[index];
                self.geng.draw_2d(
                    framebuffer,
                    camera,
                    &draw_2d::Segment::new(Segment(p1, p2), 0.2, Rgba::RED),
                );
            }
        }
    }

    fn handle_event(&mut self, event: geng::Event) {
        match event {
            geng::Event::MouseDown {
                position,
                button: geng::MouseButton::Left,
            } if self.args.editor => {
                self.start_drag = Some(
                    self.level.snap(
                        self.camera
                            .screen_to_world(self.framebuffer_size, position.map(|x| x as f32)),
                    ),
                );
            }
            geng::Event::MouseUp {
                position,
                button: geng::MouseButton::Left,
            } if self.args.editor => {
                if let Some(start) = self.start_drag.take() {
                    let end = self.level.snap(
                        self.camera
                            .screen_to_world(self.framebuffer_size, position.map(|x| x as f32)),
                    );
                    if (start - end).len() > SNAP_DISTANCE {
                        self.level.segments.push([start, end]);
                    }
                }
            }
            geng::Event::MouseDown {
                position,
                button: geng::MouseButton::Right,
            } if self.args.editor => {
                if let Some(index) = self.level.hovered_segment(
                    self.camera
                        .screen_to_world(self.framebuffer_size, position.map(|x| x as f32)),
                ) {
                    self.level.segments.remove(index);
                }
            }
            geng::Event::KeyDown { key: geng::Key::S }
                if self.geng.window().is_key_pressed(geng::Key::LCtrl) && self.args.editor => {}
            _ => {
                self.level.save(run_dir().join("level.json"));
            }
        }
    }
}

#[derive(clap::Parser)]
pub struct Args {
    #[clap(long)]
    editor: bool,
    #[clap(flatten)]
    geng: geng::CliArgs,
}

fn main() {
    logger::init().unwrap();
    geng::setup_panic_handler();
    let args: Args = clap::Parser::parse();
    let geng = Geng::new_with(geng::ContextOptions {
        title: "Coots".to_owned(),
        ..geng::ContextOptions::from_args(&args.geng)
    });
    geng::run(
        &geng,
        geng::LoadingScreen::new(&geng, geng::EmptyLoadingScreen, {
            let geng = geng.clone();
            async move {
                let assets: Assets = geng
                    .load_asset(run_dir().join("assets"))
                    .await
                    .expect("Failed to load assets");
                let assets = Rc::new(assets);
                let config: Config = geng
                    .load_asset(run_dir().join("config.json"))
                    .await
                    .expect("Failed to load config");
                let config = Rc::new(config);
                let level: Level = geng
                    .load_asset(run_dir().join("level.json"))
                    .await
                    .expect("Failed to load level");
                Game::new(&geng, &assets, level, &config, args)
            }
        }),
    );
}
