use geng::prelude::*;

#[derive(geng::Assets, Deserialize)]
#[asset(json)]
pub struct Level {
    segments: Vec<[vec2<f32>; 2]>,
}

#[derive(geng::Assets)]
pub struct Assets {
    level: Level,
}

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
}

impl Game {
    pub fn new(geng: &Geng, assets: &Rc<Assets>, config: &Rc<Config>) -> Self {
        Self {
            geng: geng.clone(),
            assets: assets.clone(),
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
        for &[p1, p2] in &self.assets.level.segments {
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
        for &[p1, p2] in &self.assets.level.segments {
            self.geng.draw_2d(
                framebuffer,
                camera,
                &draw_2d::Segment::new(Segment(p1, p2), 0.1, Rgba::WHITE),
            );
        }
    }
}

#[derive(clap::Parser)]
struct Args {
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
                Game::new(&geng, &assets, &config)
            }
        }),
    );
}
