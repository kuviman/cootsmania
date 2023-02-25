use geng::prelude::*;

mod bots;
mod game;
mod interop;
mod interpolation;
#[cfg(not(target_arch = "wasm32"))]
mod server;
mod ui;

use interop::*;
use interpolation::*;
use ui::*;

#[derive(geng::Assets, Serialize, Deserialize)]
#[asset(json)]
pub struct Level {
    segments: Vec<[vec2<f32>; 2]>,
    cat_locations: Vec<vec2<f32>>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct Track {
    pub from: usize,
    pub to: usize,
}

impl Level {
    pub fn random_track(&self) -> Track {
        self.random_track_from(thread_rng().gen_range(0..self.cat_locations.len()))
    }
    pub fn random_track_from(&self, from: usize) -> Track {
        let mut to = thread_rng().gen_range(0..self.cat_locations.len() - 1);
        if to >= from {
            to += 1;
        }
        Track { from, to }
    }
}

#[derive(Deserialize)]
pub struct SfxConfig {
    pub min: f32,
    pub max: f32,
    pub volume: f32,
}

impl SfxConfig {
    pub fn get(&self, value: f32) -> f64 {
        (((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) * self.volume) as f64
    }
}

#[derive(geng::Assets, Deserialize)]
#[asset(json)]
pub struct Config {
    pub cat_location_text: Vec<String>,
    pub min_players: usize,
    pub zoom_speed: f32,
    pub nameplate_size: f32,
    pub nameplate_outline_size: f32,
    pub max_name_len: usize,
    pub new_session_time: f32,
    pub rotation_speed: f32,
    pub acceleration: f32,
    pub backward_acceleration: f32,
    pub deceleration: f32,
    pub drift_deceleration: f32,
    pub player_radius: f32,
    pub max_speed: f32,
    pub max_backward_speed: f32,
    pub collision_bounciness: f32,
    pub camera_speed: f32,
    pub cat_move_time: i32,
    pub cat_move_time_change: f32,
    pub camera_fov: f32,
    pub arrow_size: f32,
    pub player_direction_scale: vec2<f32>,
    pub replay_fps: f32,
    pub server_recordings: bool,
    pub map_scale: f32,
    pub elimination_ratio: f32,
    pub bounce_sfx: SfxConfig,
    pub drift_sfx: SfxConfig,
    pub drift_sfx_pitch: SfxConfig,
    pub drift_speed_change: f64,
    pub music_volume: f32,
    pub particle_size: f32,
    pub drift_particles: f32,
    pub particle_drag: f32,
    pub particle_rng: f32,
    pub particle_lifespan: f32,
    pub particle_alpha: f32,
    pub forward_sfx: SfxConfig,
    pub forward_sfx_pitch: SfxConfig,
    pub forward_speed_change: f64,
}

#[derive(clap::Parser)]
pub struct Args {
    #[clap(long)]
    pub server: Option<String>,
    #[clap(long)]
    pub connect: Option<String>,
    #[clap(long)]
    pub editor: bool,
    #[clap(flatten)]
    pub geng: geng::CliArgs,
}

fn main() {
    logger::init().unwrap();
    geng::setup_panic_handler();
    let mut args: Args = clap::Parser::parse();

    if args.connect.is_none() && args.server.is_none() {
        if cfg!(target_arch = "wasm32") {
            args.connect = Some(
                option_env!("CONNECT")
                    .expect("Set CONNECT compile time env var")
                    .to_owned(),
            );
        } else {
            args.server = Some("127.0.0.1:1155".to_owned());
            args.connect = Some("ws://127.0.0.1:1155".to_owned());
        }
    }

    if args.server.is_some() && args.connect.is_none() {
        #[cfg(not(target_arch = "wasm32"))]
        geng::net::Server::new(server::App::new(), args.server.as_deref().unwrap()).run();
    } else {
        #[cfg(not(target_arch = "wasm32"))]
        let server = if let Some(addr) = &args.server {
            let server = geng::net::Server::new(server::App::new(), addr);
            let server_handle = server.handle();
            let server_thread = std::thread::spawn(move || {
                server.run();
            });
            Some((server_handle, server_thread))
        } else {
            None
        };

        let geng = Geng::new_with(geng::ContextOptions {
            title: "Coots".to_owned(),
            target_ui_resolution: Some(vec2(20.0, 10.0) * 1.2),
            ..geng::ContextOptions::from_args(&args.geng)
        });
        geng.set_ui_theme({
            let mut theme = geng::ui::Theme::light(&geng);
            theme.text_size = 1.0;
            theme
        });
        geng::run(
            &geng,
            geng::LoadingScreen::new(&geng, geng::EmptyLoadingScreen, {
                let geng = geng.clone();
                async move {
                    let assets: game::Assets = geng
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
                    let connection =
                        geng::net::client::connect(args.connect.as_deref().unwrap()).await;
                    game::Game::new(&geng, &assets, level, &config, connection, args)
                }
            }),
        );

        #[cfg(not(target_arch = "wasm32"))]
        if let Some((server_handle, server_thread)) = server {
            server_handle.shutdown();
            server_thread.join().unwrap();
        }
    }
}
