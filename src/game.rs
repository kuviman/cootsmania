use super::*;

const SNAP_DISTANCE: f32 = 0.2;

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
        info!("Saving the level");
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
pub struct SfxAssets {
    #[asset(ext = "mp3")]
    pub bounce: geng::Sound,
    #[asset(ext = "mp3")]
    pub countdown: geng::Sound,
    #[asset(ext = "mp3", postprocess = "make_looped")]
    pub drift: geng::Sound,
    #[asset(ext = "mp3")]
    pub eliminated: geng::Sound,
    #[asset(ext = "mp3")]
    pub game_start: geng::Sound,
    #[asset(ext = "mp3")]
    pub new_round: geng::Sound,
    #[asset(ext = "mp3")]
    pub qualified: geng::Sound,
    #[asset(ext = "mp3")]
    pub victory: geng::Sound,
    #[asset(ext = "mp3", postprocess = "make_looped")]
    pub forward_move: geng::Sound,
}

#[derive(geng::Assets)]
pub struct UiSfxAssets {
    #[asset(ext = "mp3")]
    pub click: geng::Sound,
    #[asset(ext = "mp3")]
    pub hover: geng::Sound,
    #[asset(ext = "mp3")]
    pub slider: geng::Sound,
}

#[derive(geng::Assets)]
pub struct UiAssets {
    pub sfx: UiSfxAssets,
    telecam_on: ugli::Texture,
    telecam_off: ugli::Texture,
    play_unhovered: ugli::Texture,
    practice: ugli::Texture,
    practice_unhovered: ugli::Texture,
    background: ugli::Texture,
    color: ugli::Texture,
    title: ugli::Texture,
    instructions: ugli::Texture,
    play: ugli::Texture,
    left: ugli::Texture,
    right: ugli::Texture,
    settings: ugli::Texture,
    volume: ugli::Texture,
    slider_line: ugli::Texture,
    volume_slider_line: ugli::Texture,
    slider_knob: ugli::Texture,
    bots: ugli::Texture,
    spectators: ugli::Texture,
    players_left: ugli::Texture,
    time: ugli::Texture,
    music_check: ugli::Texture,
    music_uncheck: ugli::Texture,
    names_check: ugli::Texture,
    names_uncheck: ugli::Texture,
}

#[derive(geng::Assets)]
pub struct Shaders {
    foo: ugli::Program,
    outline: ugli::Program,
    texture_instancing: ugli::Program,
}

#[derive(geng::Assets)]
pub struct Assets {
    pub bounce_particle: ugli::Texture,
    pub shaders: Shaders,
    pub sfx: SfxAssets,
    map_floor: ugli::Texture,
    map_furniture_front: ugli::Texture,
    map_furniture_back: ugli::Texture,
    coots: ugli::Texture,
    arrow: ugli::Texture,
    #[asset(load_with = "load_player_assets(&geng, base_path.join(\"player\"))")]
    player: Vec<ugli::Texture>,
    car: ugli::Texture,
    car_color: ugli::Texture,
    pub ui: UiAssets,
    #[asset(path = "music/Ludwig23_Medium.mp3", postprocess = "make_looped")]
    music: geng::Sound,
    #[asset(
        path = "music/Ludwig23_MainMenu_Faster.mp3",
        postprocess = "make_looped"
    )]
    menu_music: geng::Sound,
    #[asset(path = "font/Pangolin-Regular.ttf")]
    font: Rc<geng::Font>,
    particle: ugli::Texture,
}

fn make_looped(sound: &mut geng::Sound) {
    sound.looped = true;
}

fn make_each_looped(sounds: &mut [geng::Sound]) {
    for sound in sounds {
        make_looped(sound);
    }
}

async fn load_player_assets(
    geng: &Geng,
    path: impl AsRef<std::path::Path>,
) -> anyhow::Result<Vec<ugli::Texture>> {
    let path = path.as_ref();
    let json: String = geng.load_asset(path.join("_list.json")).await?;
    let list: Vec<String> = serde_json::from_str(&json)?;
    future::join_all(
        list.into_iter()
            .map(|name| geng.load_asset(path.join(format!("{name}.png")))),
    )
    .await
    .into_iter()
    .collect()
}

pub struct PlayerInput {
    rotate: f32,     // -1 .. 1
    accelerate: f32, // -1 .. 1
}

type Connection = geng::net::client::Connection<ServerMessage, ClientMessage>;

struct RemotePlayer {
    skin: usize,
    color: f32,
    pos: Interpolated<vec2<f32>>,
    rot: f32,
}

impl RemotePlayer {
    fn new(player: Player) -> Self {
        Self {
            color: player.color,
            skin: player.skin,
            pos: Interpolated::new(player.pos, player.vel),
            rot: player.rot,
        }
    }
    fn server_update(&mut self, upd: Player) {
        self.skin = upd.skin;
        self.color = upd.color;
        upd.pos.map(|x| assert!(x.is_finite()));
        self.pos.server_update(upd.pos, upd.vel);
        self.rot = upd.rot;
    }
    fn update(&mut self, delta_time: f32) {
        self.pos.update(delta_time);
    }

    fn get(&self) -> Player {
        Player {
            color: self.color,
            skin: self.skin,
            pos: self.pos.get(),
            vel: self.pos.get_derivative(),
            rot: self.rot,
        }
    }
}

struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    rot: f32,
    t: f32,
}

struct Particles {
    geng: Geng,
    config: Rc<Config>,
    assets: Rc<Assets>,
    data: Vec<Particle>,
}

impl Particles {
    fn new(geng: &Geng, config: &Rc<Config>, assets: &Rc<Assets>) -> Self {
        Self {
            assets: assets.clone(),
            config: config.clone(),
            geng: geng.clone(),
            data: default(),
        }
    }
    fn push(&mut self, pos: vec2<f32>, vel: vec2<f32>) {
        self.data.push(Particle {
            pos,
            vel: thread_rng().gen_circle(vel, self.config.particle_rng),
            t: 0.0,
            rot: thread_rng().gen_range(0.0..2.0 * f32::PI),
        })
    }
    fn update(&mut self, delta_time: f32) {
        for particle in &mut self.data {
            particle.vel -= particle
                .vel
                .clamp_len(..=self.config.particle_drag * delta_time);
            particle.pos += particle.vel * delta_time;
            particle.t += delta_time / self.config.particle_lifespan;
        }
        self.data.retain(|particle| particle.t < 1.0);
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Winner {
    None,
    Me,
    Other(Id),
}

#[derive(ugli::Vertex)]
struct TextureInstance {
    i_color: Rgba<f32>,
    i_mat: mat3<f32>,
}

pub struct Game {
    ready: bool,
    spectate_zoomed_in: bool,
    texture_instances: RefCell<HashMap<*const ugli::Texture, ugli::VertexBuffer<TextureInstance>>>,
    winner: Option<Winner>,
    geng: Geng,
    color: f32,
    assets: Rc<Assets>,
    config: Rc<Config>,
    connection: Connection,
    player: Option<Player>,
    camera: geng::Camera2d,
    level: Level,
    args: Args,
    start_drag: Option<vec2<f32>>,
    names: HashMap<Id, String>,
    framebuffer_size: vec2<f32>,
    remote_players: HashMap<Id, RemotePlayer>,
    cat_move_time: f32,
    text: Option<(String, f32)>,
    text2: Option<(String, f32)>,
    skin: usize,
    in_settings: bool,
    volume: f64,
    round: Round,
    numbers: Numbers,
    name: String,
    spectating: bool,
    music_on: bool,
    music_menu: bool,
    music: Option<geng::SoundEffect>,
    drift_sfx: geng::SoundEffect,
    forward_sfx: geng::SoundEffect,
    next_drift_particle: f32,
    drift_particles: Particles,
    bounce_particles: Particles,
    t: f32,
    show_player_names: bool,
    next_player_update: f32,
    quad_geometry: ugli::VertexBuffer<draw_2d::Vertex>,
    practice: Option<usize>,
    telecam: bool,
    gilrs: gilrs::Gilrs,
    active_gamepad: Option<gilrs::GamepadId>,
    round_countdown: f32,
}

impl Game {
    fn draw_particles(
        &self,
        particles: &Particles,
        texture: &ugli::Texture,
        color: Rgba<f32>,
        framebuffer: &mut ugli::Framebuffer,
        camera: &geng::Camera2d,
    ) {
        ugli::draw(
            framebuffer,
            &self.assets.shaders.texture_instancing,
            ugli::DrawMode::TriangleFan,
            ugli::instanced(
                &self.quad_geometry,
                &ugli::VertexBuffer::new_dynamic(
                    self.geng.ugli(),
                    particles
                        .data
                        .iter()
                        .map(|particle| TextureInstance {
                            i_color: {
                                let mut color = color;
                                color.a *= 1.0 - particle.t;
                                color
                            },
                            i_mat: mat3::translate(particle.pos)
                                * mat3::scale_uniform(self.config.particle_size)
                                * mat3::rotate(particle.rot),
                        })
                        .collect(),
                ),
            ),
            (
                ugli::uniforms! {
                    u_texture: texture,
                },
                geng::camera2d_uniforms(camera, framebuffer.size().map(|x| x as f32)),
            ),
            ugli::DrawParameters {
                blend_mode: Some(ugli::BlendMode::straight_alpha()),
                ..default()
            },
        );
    }
    pub fn new(
        geng: &Geng,
        assets: &Rc<Assets>,
        level: Level,
        config: &Rc<Config>,
        mut connection: Connection,
        args: Args,
    ) -> Self {
        connection.send(ClientMessage::Ping);
        let volume = preferences::load("volume").unwrap_or(0.5);
        geng.audio().set_volume(volume);
        let music_on: bool = preferences::load("music_on").unwrap_or(true);
        let name = preferences::load("name").unwrap_or(String::new());
        connection.send(ClientMessage::Name(name.clone()));
        let skin = preferences::load("skin").unwrap_or_else(|| {
            let skin: usize = thread_rng().gen_range(0..assets.player.len());
            preferences::save("skin", &skin);
            skin
        });
        let color = preferences::load("color").unwrap_or_else(|| {
            let color: f32 = thread_rng().gen();
            preferences::save("color", &color);
            color
        });
        Self {
            spectate_zoomed_in: false,
            ready: false,
            music_menu: true,
            quad_geometry: ugli::VertexBuffer::new_static(
                geng.ugli(),
                vec![
                    draw_2d::Vertex {
                        a_pos: vec2(0.0, 0.0),
                    },
                    draw_2d::Vertex {
                        a_pos: vec2(1.0, 0.0),
                    },
                    draw_2d::Vertex {
                        a_pos: vec2(1.0, 1.0),
                    },
                    draw_2d::Vertex {
                        a_pos: vec2(0.0, 1.0),
                    },
                ],
            ),
            texture_instances: default(),
            color,
            text2: None,
            music_on,
            music: None,
            geng: geng.clone(),
            assets: assets.clone(),
            level,
            connection,
            active_gamepad: None,
            config: config.clone(),
            player: args.editor.then_some(Player {
                skin,
                color,
                pos: vec2::ZERO,
                vel: vec2::ZERO,
                rot: 0.0,
            }),
            camera: geng::Camera2d {
                center: vec2::ZERO,
                rotation: 0.0,
                fov: config.camera_fov,
            },
            spectating: !args.editor,
            args,
            start_drag: None,
            framebuffer_size: vec2(1.0, 1.0),
            remote_players: default(),
            cat_move_time: 0.0,
            text: None,
            skin,
            in_settings: true,
            volume,
            numbers: Numbers {
                players_left: 0,
                spectators: 0,
                bots: 0,
                qualified: 0,
            },
            round: Round {
                num: 1,
                track: Track { from: 0, to: 1 },
                to_be_qualified: 0,
            },
            names: default(),
            name,
            drift_sfx: {
                let mut effect = assets.sfx.drift.effect();
                effect.set_volume(0.0);
                effect.play();
                effect
            },
            forward_sfx: {
                let mut effect = assets.sfx.forward_move.effect();
                effect.set_volume(0.0);
                effect.play();
                effect
            },
            next_drift_particle: 0.0,
            drift_particles: Particles::new(geng, config, assets),
            bounce_particles: Particles::new(geng, config, assets),
            winner: None,
            t: 0.0,
            next_player_update: 0.0,
            show_player_names: preferences::load("show_player_names").unwrap_or(true),
            practice: None,
            telecam: true,
            gilrs: gilrs::Gilrs::new().unwrap(),
            round_countdown: 0.0,
        }
    }

    fn update_connection(&mut self) {
        while let Some(message) = self.connection.try_recv() {
            let message = message.unwrap();
            match &message {
                ServerMessage::Pong => {}
                ServerMessage::UpdatePlayer(..) => {}
                _ => debug!("{message:?}"),
            }
            match message {
                ServerMessage::YourName(name) => {
                    self.name = name;
                }
                ServerMessage::Name(id, name) => {
                    self.names.insert(id, name);
                }
                ServerMessage::Pong => {
                    self.connection.send(ClientMessage::Ping);
                    if let Some(player) = &self.player {
                        if self.practice.is_none() {
                            self.connection
                                .send(ClientMessage::UpdatePlayer(player.clone()));
                        }
                    }
                }
                ServerMessage::UpdatePlayer(id, player) => match player {
                    Some(player) => match self.remote_players.entry(id) {
                        std::collections::hash_map::Entry::Occupied(mut entry) => {
                            entry.get_mut().server_update(player);
                        }
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            entry.insert(RemotePlayer::new(player));
                        }
                    },
                    None => {
                        self.remote_players.remove(&id);
                    }
                },
                ServerMessage::Disconnect(id) => {
                    self.remote_players.remove(&id);
                }
                ServerMessage::YouHaveBeenEliminated => {
                    if !self.args.editor && self.practice.is_none() {
                        self.player = None;
                        self.text2 = Some(("You have been eliminated".to_owned(), -2.0));
                        self.spectating = true;
                        self.spectate_zoomed_in = false;
                        self.assets.sfx.eliminated.play();
                    }
                }
                ServerMessage::YouHaveBeenRespawned(pos) => {
                    if !self.args.editor && self.practice.is_none() {
                        self.player = Some(Player {
                            skin: self.skin,
                            color: self.color,
                            pos,
                            vel: vec2::ZERO,
                            rot: thread_rng().gen_range(0.0..2.0 * f32::PI),
                        });
                        self.spectating = false;
                        self.assets.sfx.new_round.play();
                        // self.text = Some(("GO".to_owned(), 0.0));
                    }
                }
                ServerMessage::RoundStarted => {
                    self.cat_move_time = self.config.cat_move_time as f32;
                }
                ServerMessage::Numbers(numbers) => {
                    self.numbers = numbers;
                }
                ServerMessage::NewRound(round) => {
                    if !self.args.editor && self.practice.is_none() {
                        self.player = None;
                    }
                    self.winner = None;
                    self.cat_move_time = 3.0;
                    self.remote_players.clear();
                    if self.ready {
                        self.assets.sfx.countdown.play().set_volume(3.0);
                    }
                    self.round = round;
                    self.text = Some((
                        if self.round.num == 0 {
                            "Warmup round!".to_string()
                        } else {
                            format!("Round {}!", self.round.num)
                        } + "\n"
                            + &self.config.cat_location_text[self.round.track.to],
                        -2.0,
                    ));
                    self.round_countdown = 3.0;
                }
                ServerMessage::YouHaveBeenQualified => {
                    if !self.args.editor && self.practice.is_none() {
                        self.player = None;
                        self.text = Some(("QUALIFIED!!!".to_owned(), 0.0));
                        if self.ready {
                            self.assets.sfx.qualified.play();
                        }
                    }
                }
                ServerMessage::YouAreWinner => {
                    if self.ready {
                        self.assets.sfx.victory.play();
                    }
                    self.winner = Some(Winner::Me);
                    self.text = Some(("You are the champion!".to_owned(), -2.0));
                }
                ServerMessage::Winner(winner) => match winner {
                    Some(id) => {
                        if self.ready {
                            self.assets.sfx.victory.play();
                        }
                        self.winner = Some(Winner::Other(id));
                        self.text = Some(("Champion!".to_owned(), -2.0));
                    }
                    None => {
                        if self.ready {
                            self.assets.sfx.eliminated.play();
                        }
                        self.winner = Some(Winner::None);
                        self.text = Some(("Nobody won :(".to_owned(), -2.0));
                    }
                },
            }
        }
    }

    fn draw_texture_instances(&self, framebuffer: &mut ugli::Framebuffer, camera: &geng::Camera2d) {
        let mut texture_instances = self.texture_instances.borrow_mut();
        let camera_uniforms = geng::camera2d_uniforms(camera, self.framebuffer_size);
        for texture in itertools::chain![
            [&self.assets.car, &self.assets.car_color],
            &self.assets.player
        ] {
            if let Some(instances) = texture_instances.get_mut(&(texture as *const ugli::Texture)) {
                ugli::draw(
                    framebuffer,
                    &self.assets.shaders.texture_instancing,
                    ugli::DrawMode::TriangleFan,
                    ugli::instanced(&self.quad_geometry, &*instances),
                    (
                        ugli::uniforms! {
                            u_texture: texture,
                        },
                        &camera_uniforms,
                    ),
                    ugli::DrawParameters {
                        blend_mode: Some(ugli::BlendMode::straight_alpha()),
                        ..default()
                    },
                );
                instances.clear();
            }
        }
    }

    fn add_texture_instance(&self, texture: &ugli::Texture, color: Rgba<f32>, matrix: mat3<f32>) {
        let mut instances = self.texture_instances.borrow_mut();
        instances
            .entry(texture as *const ugli::Texture)
            .or_insert_with(|| ugli::VertexBuffer::new_dynamic(self.geng.ugli(), vec![]))
            .push(TextureInstance {
                i_color: color,
                i_mat: matrix,
            });
    }

    fn draw_player_car(
        &self,
        framebuffer: &mut ugli::Framebuffer,
        camera: &geng::Camera2d,
        player: &Player,
        id: Option<Id>, // None = me
    ) {
        let alpha = if id.is_some() { 0.5 } else { 1.0 };
        self.add_texture_instance(
            &self.assets.car,
            Rgba::new(1.0, 1.0, 1.0, alpha),
            mat3::translate(player.pos + vec2(0.0, 0.4))
                * mat3::scale(self.config.player_direction_scale * self.config.player_radius * 0.7)
                * mat3::rotate(player.rot),
        );

        self.add_texture_instance(
            &self.assets.car_color,
            batbox::color::Hsva::new(player.color, 1.0, 1.0, alpha).into(),
            mat3::translate(player.pos + vec2(0.0, 0.4))
                * mat3::scale(self.config.player_direction_scale * self.config.player_radius * 0.7)
                * mat3::rotate(player.rot),
        );
    }

    fn draw_player_body(
        &self,
        framebuffer: &mut ugli::Framebuffer,
        camera: &geng::Camera2d,
        player: &Player,
        id: Option<Id>, // None = me
    ) {
        let alpha = if id.is_some() { 0.5 } else { 1.0 };
        if let Some(texture) = self.assets.player.get(player.skin) {
            self.add_texture_instance(
                texture,
                Rgba::new(1.0, 1.0, 1.0, alpha),
                mat3::translate(player.pos + vec2(0.0, self.config.player_radius))
                    * mat3::scale_uniform(self.config.player_radius / std::f32::consts::SQRT_2),
            );
        }
    }

    fn draw_player_outline(
        &self,
        framebuffer: &mut ugli::Framebuffer,
        camera: &geng::Camera2d,
        player: &Player,
    ) {
        let background_pos = Aabb2::point(vec2::ZERO).extend_symmetric({
            let size = self.assets.map_floor.size().map(|x| x as f32);
            vec2(size.x / size.y, 1.0) * self.config.map_scale
        });
        let mut draw_texture = |texture: &ugli::Texture, matrix: mat3<f32>, car: bool| {
            ugli::draw(
                framebuffer,
                &self.assets.shaders.outline,
                ugli::DrawMode::TriangleFan,
                &self.quad_geometry,
                (
                    ugli::uniforms! {
                        u_color: {
                            let c: Rgba<f32> = Hsva::new(player.color, 1.0, 1.0,1.0).into();
                            c
                        },
                        u_texture: texture,
                        u_furniture: &self.assets.map_furniture_front,
                        u_matrix: matrix,
                        u_background_pos: background_pos.bottom_left(),
                        u_background_size: background_pos.size(),
                    },
                    geng::camera2d_uniforms(camera, self.framebuffer_size),
                ),
                ugli::DrawParameters {
                    stencil_mode: if car {
                        None
                    } else {
                        Some(ugli::StencilMode::always(ugli::FaceStencilMode {
                            test: ugli::StencilTest {
                                condition: ugli::Condition::NotEqual,
                                reference: 1,
                                mask: 0xff,
                            },
                            op: ugli::StencilOp::always(ugli::StencilOpFunc::Keep),
                        }))
                    },
                    blend_mode: Some(ugli::BlendMode::straight_alpha()),
                    ..default()
                },
            );
        };
        if let Some(texture) = self.assets.player.get(player.skin) {
            draw_texture(
                texture,
                mat3::translate(player.pos + vec2(0.0, self.config.player_radius))
                    * mat3::scale_uniform(self.config.player_radius / std::f32::consts::SQRT_2),
                false,
            );
        }
        draw_texture(
            &self.assets.car,
            mat3::translate(player.pos + vec2(0.0, 0.4))
                * mat3::scale(self.config.player_direction_scale * self.config.player_radius * 0.7)
                * mat3::rotate(player.rot),
            true,
        );
        draw_texture(
            &self.assets.car_color,
            mat3::translate(player.pos + vec2(0.0, 0.4))
                * mat3::scale(self.config.player_direction_scale * self.config.player_radius * 0.7)
                * mat3::rotate(player.rot),
            true,
        );
    }

    fn draw_player_name(
        &self,
        framebuffer: &mut ugli::Framebuffer,
        camera: &geng::Camera2d,
        player: &Player,
        id: Option<Id>, // None = me
    ) {
        if self.show_player_names {
            self.assets.font.draw_with_outline(
                framebuffer,
                camera,
                match id {
                    None => &self.name,
                    Some(id) => self.names.get(&id).map(|s| s.as_str()).unwrap_or(""),
                },
                player.pos + vec2(0.0, self.config.player_radius * 2.0),
                geng::TextAlign::CENTER,
                self.config.nameplate_size,
                Rgba::WHITE,
                self.config.nameplate_outline_size,
                Rgba::BLACK,
            );
        }
    }

    fn update_my_player(&mut self, delta_time: f32) {
        if let Some(to) = self.practice {
            if let Some(player) = &self.player {
                let coots = self.level.cat_locations[to];
                if (player.pos - coots).len() < self.config.player_radius * 2.0
                    && player.vel.len() < 1e-5
                {
                    self.practice = Some(loop {
                        let new = thread_rng().gen_range(0..self.level.cat_locations.len());
                        if new != to {
                            break new;
                        }
                    });
                    self.assets.sfx.new_round.play();
                }
            }
        }

        let players_aabb = if self.remote_players.is_empty() {
            None
        } else {
            Some(Aabb2::points_bounding_box(
                self.remote_players.values().map(|player| player.pos.get()),
            ))
        };

        if self.spectating {
            if let Some(Winner::Other(id)) = self.winner {
                if let Some(p) = self.remote_players.get(&id) {
                    p.get().pos.map(|x| assert!(x.is_finite()));
                    self.camera.center += (p.get().pos - self.camera.center)
                        * (self.config.camera_speed * delta_time).min(1.0);
                }
            } else if self.spectate_zoomed_in {
                let mut dir = vec2::ZERO;
                if self.geng.window().is_key_pressed(geng::Key::Left)
                    || self.geng.window().is_key_pressed(geng::Key::A)
                {
                    dir.x -= 1.0;
                }
                if self.geng.window().is_key_pressed(geng::Key::Right)
                    || self.geng.window().is_key_pressed(geng::Key::D)
                {
                    dir.x += 1.0;
                }
                if self.geng.window().is_key_pressed(geng::Key::Down)
                    || self.geng.window().is_key_pressed(geng::Key::S)
                {
                    dir.y -= 1.0;
                }
                if self.geng.window().is_key_pressed(geng::Key::Up)
                    || self.geng.window().is_key_pressed(geng::Key::W)
                {
                    dir.y += 1.0;
                }
                self.camera.center += dir * 20.0 * delta_time;
                self.camera.center = self.camera.center.clamp_aabb(Aabb2::points_bounding_box(
                    self.level.segments.iter().copied().flatten(),
                ));
            } else {
                let target_center = if let Some(aabb) = players_aabb.filter(|_| self.telecam) {
                    aabb.center()
                } else {
                    vec2::ZERO
                };
                self.camera.center += (target_center - self.camera.center)
                    * (self.config.camera_speed * delta_time).min(1.0);
            }
        }

        let target_fov = if !self.spectating {
            self.config.camera_fov
        } else if let Some(Winner::Other(_)) = self.winner {
            self.config.camera_fov
        } else if self.telecam {
            // pgorley says to add more unwraps
            players_aabb.map_or(self.config.map_scale * 2.0, |aabb| {
                partial_max(
                    aabb.height() + self.config.camera_fov,
                    (aabb.width() + self.config.camera_fov) * self.framebuffer_size.y
                        / self.framebuffer_size.x,
                )
            })
        } else if self.spectate_zoomed_in {
            self.config.camera_fov
        } else {
            self.config.map_scale * 2.0
        };
        self.camera.fov +=
            (target_fov - self.camera.fov) * (self.config.zoom_speed * delta_time).min(1.0);

        let player = match &mut self.player {
            Some(player) => player,
            None => {
                self.drift_sfx.set_volume(0.0);
                self.forward_sfx.set_volume(0.0);
                return;
            }
        };

        self.next_player_update -= delta_time;
        while self.next_player_update < 0.0 {
            let delta_time = 1.0 / 200.0;
            self.next_player_update += delta_time;

            while let Some(event) = self.gilrs.next_event() {
                self.active_gamepad = Some(event.id);
            }

            let input = PlayerInput {
                rotate: {
                    let mut value: f32 = 0.0;
                    if self.geng.window().is_key_pressed(geng::Key::Left)
                        || self.geng.window().is_key_pressed(geng::Key::A)
                    {
                        value += 1.0;
                    }
                    if self.geng.window().is_key_pressed(geng::Key::Right)
                        || self.geng.window().is_key_pressed(geng::Key::D)
                    {
                        value -= 1.0;
                    }
                    if let Some(gamepad) = self
                        .active_gamepad
                        .and_then(|id| self.gilrs.connected_gamepad(id))
                    {
                        if let Some(axis) = gamepad.axis_data(gilrs::Axis::LeftStickX) {
                            value -= axis.value();
                        }
                    }
                    value.clamp(-1.0, 1.0)
                },
                accelerate: {
                    let mut value: f32 = 0.0;
                    if self.geng.window().is_key_pressed(geng::Key::Down)
                        || self.geng.window().is_key_pressed(geng::Key::S)
                    {
                        value -= 1.0;
                    }
                    if self.geng.window().is_key_pressed(geng::Key::Up)
                        || self.geng.window().is_key_pressed(geng::Key::W)
                    {
                        value += 1.0;
                    }
                    if let Some(gamepad) = self
                        .active_gamepad
                        .and_then(|id| self.gilrs.connected_gamepad(id))
                    {
                        if let Some(button) = gamepad.button_data(gilrs::Button::LeftTrigger) {
                            value -= button.value();
                        }
                        if let Some(button) = gamepad.button_data(gilrs::Button::LeftTrigger2) {
                            value -= button.value();
                        }
                        if let Some(button) = gamepad.button_data(gilrs::Button::RightTrigger) {
                            value += button.value();
                        }
                        if let Some(button) = gamepad.button_data(gilrs::Button::RightTrigger2) {
                            value += button.value();
                        }
                    }
                    value.clamp(-1.0, 1.0)
                },
            };

            player.rot += input.rotate * self.config.rotation_speed * delta_time;
            let dir = vec2(1.0, 0.0).rotate(player.rot);

            let mut forward_vel = vec2::dot(dir, player.vel);
            let (target_forward_vel, forward_acceleration) = if input.accelerate > 0.0 {
                let target_forward_vel = input.accelerate * self.config.max_speed;
                let forward_acceleration = if target_forward_vel > forward_vel {
                    if forward_vel < 0.0 {
                        self.config.deceleration
                    } else {
                        self.config.acceleration
                    }
                } else {
                    -self.config.deceleration
                };
                (target_forward_vel, forward_acceleration)
            } else {
                let target_forward_vel = input.accelerate * self.config.max_backward_speed;
                let forward_acceleration = if target_forward_vel < forward_vel {
                    if forward_vel > 0.0 {
                        -self.config.deceleration
                    } else {
                        -self.config.backward_acceleration
                    }
                } else {
                    self.config.deceleration
                };
                (target_forward_vel, forward_acceleration)
            };
            forward_vel += (target_forward_vel - forward_vel)
                .clamp_abs(forward_acceleration.abs() * delta_time);

            let mut drift_vel = vec2::skew(dir, player.vel);
            drift_vel -= drift_vel.clamp_abs(self.config.drift_deceleration * delta_time);

            let old_vel = player.vel;
            player.vel = dir * forward_vel + dir.rotate_90() * drift_vel;

            let drift_value = drift_vel.abs();
            self.drift_sfx
                .set_volume(self.config.drift_sfx.get(drift_value));
            self.drift_sfx.set_speed(
                (self.config.drift_sfx_pitch.get(drift_value) * 2.0 - 1.0)
                    * self.config.drift_speed_change
                    + 1.0,
            );
            self.next_drift_particle -= self.config.drift_sfx.get(drift_value) as f32 * delta_time;
            while self.next_drift_particle < 0.0 {
                self.next_drift_particle += 1.0 / self.config.drift_particles;
                self.drift_particles.push(player.pos, player.vel);
            }
            let forward_value = forward_vel.abs();
            self.forward_sfx
                .set_volume(self.config.forward_sfx.get(forward_value));
            self.forward_sfx.set_speed(
                (self.config.forward_sfx_pitch.get(forward_value) * 2.0 - 1.0)
                    * self.config.forward_speed_change
                    + 1.0,
            );

            player.pos += player.vel * delta_time;
            #[derive(PartialEq)]
            struct Collision {
                n: vec2<f32>,
                penetration: f32,
            }
            impl PartialOrd for Collision {
                fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                    Some(
                        self.penetration
                            .partial_cmp(&other.penetration)
                            .unwrap()
                            .reverse(),
                    )
                }
            }
            let mut collision = None;
            for &[p1, p2] in &self.level.segments {
                let v = -vector_from(player.pos, p1, p2);
                let penetration = self.config.player_radius - v.len();
                let n = v.normalize_or_zero();
                if penetration > 0.0 {
                    collision = partial_max(collision, Some(Collision { n, penetration }));
                }
            }
            if let Some(Collision { n, penetration }) = collision {
                player.pos += n * penetration;
                let v = vec2::dot(n, player.vel);
                let sfx_volume = self.config.bounce_sfx.get(v.abs());
                if sfx_volume > 0.0 {
                    let mut effect = self.assets.sfx.bounce.effect();
                    effect.set_speed(thread_rng().gen_range(0.8..1.2));
                    effect.set_volume(sfx_volume);
                    effect.play();
                    for _ in 0..(sfx_volume * 2.0) as usize {
                        self.bounce_particles.push(
                            player.pos - n * self.config.player_radius,
                            -player.vel * 0.5,
                        );
                    }
                }
                player.vel -= n * v * (1.0 + self.config.collision_bounciness);
            }
        }

        let cat_pos = self.level.cat_locations[self.round.track.to];
        if (player.pos - cat_pos).len() < self.config.player_radius * 2.0 && self.text.is_none() {
            self.text = Some(("STOP!".to_owned(), 0.0));
        }

        let mut target_camera_center = player.pos;
        if let Some(gamepad) = self
            .active_gamepad
            .and_then(|id| self.gilrs.connected_gamepad(id))
        {
            if let Some(axis) = gamepad.axis_data(gilrs::Axis::RightStickX) {
                target_camera_center.x += axis.value() * self.camera.fov * 0.5;
            }
            if let Some(axis) = gamepad.axis_data(gilrs::Axis::RightStickY) {
                target_camera_center.y += axis.value() * self.camera.fov * 0.5;
            }
        }

        self.camera.center += (target_camera_center - self.camera.center)
            * (self.config.camera_speed * delta_time).min(1.0);
    }

    fn draw_game(&mut self, framebuffer: &mut ugli::Framebuffer) {
        ugli::clear(framebuffer, Some(Rgba::BLACK), None, None);

        // KEKW
        if !self.camera.center.x.is_finite()
            || !self.camera.center.y.is_finite()
            || !self.camera.fov.is_finite()
        {
            self.camera = geng::Camera2d {
                center: vec2::ZERO,
                fov: 1.0,
                rotation: 0.0,
            };
        }

        let camera = &self.camera;
        let camera_aabb = camera.view_area(self.framebuffer_size).bounding_box();

        let texture_pos = Aabb2::point(vec2::ZERO).extend_symmetric({
            let size = self.assets.map_floor.size().map(|x| x as f32);
            vec2(size.x / size.y, 1.0) * self.config.map_scale
        });
        self.geng.draw_2d(
            framebuffer,
            camera,
            &draw_2d::TexturedQuad::new(texture_pos, &self.assets.map_floor),
        );
        self.geng.draw_2d(
            framebuffer,
            camera,
            &draw_2d::TexturedQuad::new(texture_pos, &self.assets.map_furniture_back),
        );

        if let Some(&pos) = self
            .level
            .cat_locations
            .get(self.practice.unwrap_or(self.round.track.to))
        {
            let mut pos = pos;
            if self.practice.is_none() {
                match self.winner {
                    Some(Winner::Me) => {
                        if let Some(player) = &self.player {
                            pos = player.pos;
                        }
                    }
                    Some(Winner::Other(id)) => {
                        if let Some(p) = self.remote_players.get(&id) {
                            pos = p.get().pos;
                        }
                    }
                    _ => {}
                }
            }
            self.geng.draw_2d(
                framebuffer,
                camera,
                &draw_2d::TexturedQuad::colored(
                    Aabb2::point(pos).extend_uniform(self.config.player_radius),
                    &self.assets.coots,
                    Rgba::new(0.0, 0.0, 0.0, 0.3),
                ),
            );
        } else {
            error!("Cat location not found!");
        }

        if self.practice.is_none() {
            for (&id, player) in &self.remote_players {
                self.draw_player_car(framebuffer, camera, &player.get(), Some(id));
            }
            self.draw_texture_instances(framebuffer, camera);
        }

        if let Some(player) = &self.player {
            self.draw_player_car(framebuffer, camera, player, None);
        }
        self.draw_texture_instances(framebuffer, camera);

        self.geng.draw_2d(
            framebuffer,
            camera,
            &draw_2d::TexturedQuad::new(texture_pos, &self.assets.map_furniture_front),
        );

        if self.practice.is_none() {
            for (&id, player) in &self.remote_players {
                self.draw_player_body(framebuffer, camera, &player.get(), Some(id));
            }
        }
        if let Some(player) = &self.player {
            self.draw_player_body(framebuffer, camera, player, None);
        }
        self.draw_texture_instances(framebuffer, camera);

        self.draw_part(
            framebuffer,
            camera,
            texture_pos,
            &self.assets.map_furniture_front,
            self.config.player_radius,
            true,
        );

        if let Some(player) = &self.player {
            self.draw_player_outline(framebuffer, camera, player);
        }

        self.draw_particles(
            &self.drift_particles,
            &self.assets.particle,
            Rgba::new(1.0, 1.0, 1.0, self.config.particle_alpha),
            framebuffer,
            camera,
        );

        self.draw_particles(
            &self.bounce_particles,
            &self.assets.bounce_particle,
            {
                let mut c: Rgba<f32> = Hsva::new(self.color, 1.0, 1.0, 1.0).into();
                c.a *= self.config.particle_alpha;
                c
            },
            framebuffer,
            camera,
        );

        if let Some(&pos) = self
            .level
            .cat_locations
            .get(self.practice.unwrap_or(self.round.track.to))
        {
            let mut pos = pos;
            if self.practice.is_none() {
                match self.winner {
                    Some(Winner::Me) => {
                        if let Some(player) = &self.player {
                            pos = player.pos;
                        }
                    }
                    Some(Winner::Other(id)) => {
                        if let Some(p) = self.remote_players.get(&id) {
                            pos = p.get().pos;
                        }
                    }
                    _ => {}
                }
            }
            self.geng.draw_2d(
                framebuffer,
                camera,
                &draw_2d::TexturedQuad::new(
                    Aabb2::point(pos + vec2(0.0, 3.0 + (self.t * 2.0).sin() * 0.2))
                        .extend_uniform(self.config.player_radius * 2.0),
                    &self.assets.coots,
                ),
            );
        }

        if self.practice.is_none() {
            for (&id, player) in self.remote_players.iter().take(50) {
                self.draw_player_name(framebuffer, camera, &player.get(), Some(id));
            }
        }
        if let Some(player) = &self.player {
            self.draw_player_name(framebuffer, camera, player, None);
        }

        if let Some(&pos) = self
            .level
            .cat_locations
            .get(self.practice.unwrap_or(self.round.track.to))
        {
            if !camera_aabb.contains(pos) {
                let mut aabb = camera_aabb.extend_uniform(-self.config.arrow_size);
                if aabb.max.x < aabb.min.x {
                    aabb.max.x = aabb.min.x;
                }
                if aabb.max.y < aabb.min.y {
                    aabb.max.y = aabb.min.y;
                }
                let arrow_pos = vec2(
                    pos.x.clamp(aabb.min.x, aabb.max.x),
                    pos.y.clamp(aabb.min.y, aabb.max.y),
                );

                let size = self.config.arrow_size * {
                    let t = (self.t / 1.0).fract();
                    if t > 0.5 {
                        1.0
                    } else {
                        fn shake(t: f32, strength: f32, vibrato: usize) -> f32 {
                            (t * f32::PI * vibrato as f32).sin() * strength + 1.0
                        }
                        fn lerp(range: Range<f32>, t: f32) -> f32 {
                            range.start * (1.0 - t) + range.end * t
                        }
                        fn ease_out_cubic(t: f32) -> f32 {
                            1.0 - (1.0 - t).powi(3)
                        }
                        fn fadeout(t: f32, value: f32) -> f32 {
                            lerp(value..1.0, ease_out_cubic(t))
                        }
                        fadeout(t, shake(t * 2.0, 0.1, 5))
                    }
                };
                self.geng.draw_2d(
                    framebuffer,
                    camera,
                    &draw_2d::TexturedQuad::unit(&self.assets.arrow)
                        .scale_uniform(size)
                        .rotate((pos - arrow_pos).arg())
                        .translate(arrow_pos),
                );
                self.geng.draw_2d(
                    framebuffer,
                    camera,
                    &draw_2d::TexturedQuad::unit(&self.assets.coots)
                        .scale_uniform(size * 0.5)
                        .translate(arrow_pos),
                );
            }
        }

        let ui_camera = &geng::Camera2d {
            center: vec2::ZERO,
            rotation: 0.0,
            fov: 10.0,
        };
        let ui_aabb = ui_camera.view_area(self.framebuffer_size).bounding_box();
        self.assets.font.draw_with_outline(
            framebuffer,
            ui_camera,
            &if let Some(to) = self.practice {
                format!("SOLO PRACTICE MODE\n{}", self.config.cat_location_text[to])
            } else if self.player.is_some() {
                self.config.cat_location_text[self.round.track.to].clone()
            } else if self.spectating {
                format!(
                    "wait for current game to finish\n{} round(s) left",
                    if self.numbers.players_left == 0 {
                        0
                    } else {
                        (self.numbers.players_left as f32).log2().ceil() as i32
                    } + if self.round.num == 0 { 1 } else { 0 }
                )
            } else {
                "wait for other players".to_owned()
            },
            vec2(0.0, -4.0),
            geng::TextAlign::CENTER,
            1.0,
            Rgba::WHITE,
            0.05,
            Rgba::BLACK,
        );
        if let Some(cat) = self.practice {
            let cat = self.level.cat_locations[cat];
            if let Some(player) = &self.player {
                if (player.pos - cat).len() < self.config.player_radius * 2.0 {
                    self.assets.font.draw_with_outline(
                        framebuffer,
                        ui_camera,
                        "STOP!",
                        vec2(0.0, 2.0),
                        geng::TextAlign::CENTER,
                        1.0,
                        Rgba::WHITE,
                        0.05,
                        Rgba::BLACK,
                    );
                }
            }
        }
        if self.practice.is_none() {
            if let Some((ref text, t)) = self.text {
                self.assets.font.draw_with_outline(
                    framebuffer,
                    ui_camera,
                    text,
                    vec2(0.0, 2.0),
                    geng::TextAlign::CENTER,
                    1.0,
                    Rgba::WHITE,
                    0.05,
                    Rgba::BLACK,
                );
            }
            if let Some((ref text, t)) = self.text2 {
                self.assets.font.draw_with_outline(
                    framebuffer,
                    ui_camera,
                    text,
                    vec2(0.0, 0.0),
                    geng::TextAlign::CENTER,
                    1.0,
                    Rgba::WHITE,
                    0.05,
                    Rgba::BLACK,
                );
            }
            if self.round.num == 0 && self.round_countdown > 0.0 {
                self.assets.font.draw_with_outline(
                    framebuffer,
                    ui_camera,
                    "GET READY",
                    vec2(0.0, -1.0),
                    geng::TextAlign::CENTER,
                    2.0,
                    Rgba::WHITE,
                    0.3,
                    Rgba::BLACK,
                );
                self.assets.font.draw_with_outline(
                    framebuffer,
                    ui_camera,
                    "GET READY",
                    vec2(0.0, -1.0),
                    geng::TextAlign::CENTER,
                    2.0,
                    Rgba::WHITE,
                    0.1,
                    Rgba::WHITE,
                );
            } else if !self.spectating && self.round_countdown > 0.0 {
                self.assets.font.draw_with_outline(
                    framebuffer,
                    ui_camera,
                    &(self.round_countdown.ceil() as i32).to_string(),
                    vec2(0.0, -1.0),
                    geng::TextAlign::CENTER,
                    2.0,
                    Rgba::WHITE,
                    0.3,
                    Rgba::BLACK,
                );
                self.assets.font.draw_with_outline(
                    framebuffer,
                    ui_camera,
                    &(self.round_countdown.ceil() as i32).to_string(),
                    vec2(0.0, -1.0),
                    geng::TextAlign::CENTER,
                    2.0,
                    Rgba::WHITE,
                    0.1,
                    Rgba::WHITE,
                );
            } else if !self.spectating && self.round_countdown > -1.0 {
                self.assets.font.draw_with_outline(
                    framebuffer,
                    ui_camera,
                    "GO",
                    vec2(0.0, 2.0),
                    geng::TextAlign::CENTER,
                    2.0,
                    Rgba::WHITE,
                    0.3,
                    Rgba::BLACK,
                );
                self.assets.font.draw_with_outline(
                    framebuffer,
                    ui_camera,
                    "GO",
                    vec2(0.0, 2.0),
                    geng::TextAlign::CENTER,
                    2.0,
                    Rgba::WHITE,
                    0.1,
                    Rgba::WHITE,
                );
            }
            let padding = 0.2;
            let font_size = 0.5;
            let outline_size = 0.03;

            // Time
            {
                let font_size = font_size * 2.0;
                self.geng.draw_2d(
                    framebuffer,
                    ui_camera,
                    &draw_2d::TexturedQuad::new(
                        Aabb2::point(ui_aabb.top_left() + vec2(padding, -padding - font_size))
                            .extend_positive(vec2(font_size, font_size)),
                        &self.assets.ui.time,
                    ),
                );
                self.assets.font.draw_with_outline(
                    framebuffer,
                    ui_camera,
                    &{
                        let millis = self.cat_move_time.max(0.0) * 1000.0;
                        let millis = millis as i64;
                        let secs = millis / 1000;
                        let millis = millis % 1000;
                        format!("{secs}:{millis}")
                    },
                    ui_aabb.top_left() + vec2(padding + font_size, -font_size - padding),
                    geng::TextAlign::LEFT,
                    font_size,
                    Rgba::WHITE,
                    outline_size,
                    Rgba::BLACK,
                );
            }

            let Numbers {
                players_left,
                spectators,
                bots,
                qualified,
            } = self.numbers;
            let to_be_qualified = self.round.to_be_qualified;

            // Qualified numbers
            self.assets.font.draw_with_outline(
                framebuffer,
                ui_camera,
                &format!("Qualified: {qualified}/{to_be_qualified}"),
                vec2(0.0, ui_aabb.max.y - font_size - padding),
                geng::TextAlign::CENTER,
                font_size,
                Rgba::WHITE,
                outline_size,
                Rgba::BLACK,
            );

            // Num of players
            {
                let font_size = font_size * 1.5;
                let numbers_width = 1.0;
                self.geng.draw_2d(
                    framebuffer,
                    ui_camera,
                    &draw_2d::TexturedQuad::unit(&self.assets.ui.players_left)
                        .translate(vec2(1.0, 1.0))
                        .scale_uniform(font_size / 2.0)
                        .translate(
                            ui_aabb.top_right()
                                - vec2(
                                    padding + numbers_width + padding + font_size,
                                    font_size + padding,
                                ),
                        ),
                );
                self.geng.draw_2d(
                    framebuffer,
                    ui_camera,
                    &draw_2d::TexturedQuad::unit(&self.assets.ui.spectators)
                        .translate(vec2(1.0, 1.0))
                        .scale_uniform(font_size / 2.0)
                        .translate(
                            ui_aabb.top_right()
                                - vec2(
                                    padding + numbers_width + padding + font_size,
                                    font_size * 2.0 + padding,
                                ),
                        ),
                );
                self.geng.draw_2d(
                    framebuffer,
                    ui_camera,
                    &draw_2d::TexturedQuad::unit(&self.assets.ui.bots)
                        .translate(vec2(1.0, 1.0))
                        .scale_uniform(font_size / 2.0)
                        .translate(
                            ui_aabb.top_right()
                                - vec2(
                                    padding + numbers_width + padding + font_size,
                                    font_size * 3.0 + padding,
                                ),
                        ),
                );
                self.assets.font.draw_with_outline(
                    framebuffer,
                    ui_camera,
                    &players_left.to_string(),
                    ui_aabb.top_right() - vec2(padding + numbers_width, font_size + padding),
                    geng::TextAlign::LEFT,
                    font_size,
                    Rgba::WHITE,
                    outline_size,
                    Rgba::BLACK,
                );
                self.assets.font.draw_with_outline(
                    framebuffer,
                    ui_camera,
                    &spectators.to_string(),
                    ui_aabb.top_right() - vec2(padding + numbers_width, font_size * 2.0 + padding),
                    geng::TextAlign::LEFT,
                    font_size,
                    Rgba::WHITE,
                    outline_size,
                    Rgba::BLACK,
                );
                self.assets.font.draw_with_outline(
                    framebuffer,
                    ui_camera,
                    &bots.to_string(),
                    ui_aabb.top_right() - vec2(padding + numbers_width, font_size * 3.0 + padding),
                    geng::TextAlign::LEFT,
                    font_size,
                    Rgba::WHITE,
                    outline_size,
                    Rgba::BLACK,
                );
            }
        }

        if self.args.editor {
            for &[p1, p2] in &self.level.segments {
                self.geng.draw_2d(
                    framebuffer,
                    camera,
                    &draw_2d::Segment::new(Segment(p1, p2), 0.1, Rgba::WHITE),
                );
            }
        }

        if self.args.editor {
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
            for &p in &self.level.cat_locations {
                self.geng.draw_2d(
                    framebuffer,
                    camera,
                    &draw_2d::Quad::new(Aabb2::point(p).extend_uniform(0.3), Rgba::GREEN),
                );
            }
        }
    }

    fn draw_part(
        &self,
        framebuffer: &mut ugli::Framebuffer,
        camera: &geng::Camera2d,
        texture_pos: Aabb2<f32>,
        texture: &ugli::Texture2d<Rgba<f32>>,
        offset: f32,
        side: bool,
    ) {
        // let offset = offset * 1.2;
        // self.geng.draw_2d(
        //     framebuffer,
        //     camera,
        //     &draw_2d::TexturedQuad::new(texture_pos, texture),
        // );

        #[derive(ugli::Vertex)]
        struct FooVertex {
            a_pos: vec2<f32>,
            a_vt: vec2<f32>,
        }
        ugli::clear(framebuffer, None, None, Some(0));
        ugli::draw(
            framebuffer,
            &self.assets.shaders.foo,
            ugli::DrawMode::Triangles,
            &ugli::VertexBuffer::new_dynamic(self.geng.ugli(), {
                let n = vec2(0.0, offset);
                let mut vs = Vec::<FooVertex>::new();
                let v = |p| FooVertex {
                    a_pos: p,
                    a_vt: vec2::ZERO,
                };
                for &[p1, p2] in &self.level.segments {
                    vs.push(v(p1));
                    vs.push(v(p2));
                    vs.push(v(p2 + n));
                    vs.push(v(p1));
                    vs.push(v(p2 + n));
                    vs.push(v(p1 + n));
                }
                vs
            }),
            (
                ugli::uniforms! {
                    u_texture: texture,
                },
                geng::camera2d_uniforms(camera, self.framebuffer_size),
            ),
            ugli::DrawParameters {
                stencil_mode: Some(ugli::StencilMode::always(ugli::FaceStencilMode {
                    test: ugli::StencilTest {
                        condition: ugli::Condition::Always,
                        reference: 1,
                        mask: 0,
                    },
                    op: ugli::StencilOp::always(ugli::StencilOpFunc::Replace),
                })),
                write_color: false,
                write_depth: false,
                ..default()
            },
        );
        ugli::draw(
            framebuffer,
            &self.assets.shaders.foo,
            ugli::DrawMode::TriangleFan,
            &ugli::VertexBuffer::new_dynamic(
                self.geng.ugli(),
                vec![
                    FooVertex {
                        a_pos: texture_pos.bottom_left(),
                        a_vt: vec2(0.0, 0.0),
                    },
                    FooVertex {
                        a_pos: texture_pos.bottom_right(),
                        a_vt: vec2(1.0, 0.0),
                    },
                    FooVertex {
                        a_pos: texture_pos.top_right(),
                        a_vt: vec2(1.0, 1.0),
                    },
                    FooVertex {
                        a_pos: texture_pos.top_left(),
                        a_vt: vec2(0.0, 1.0),
                    },
                ],
            ),
            (
                ugli::uniforms! {
                    u_texture: texture,
                },
                geng::camera2d_uniforms(camera, self.framebuffer_size),
            ),
            ugli::DrawParameters {
                stencil_mode: Some(ugli::StencilMode::always(ugli::FaceStencilMode {
                    test: ugli::StencilTest {
                        condition: if side {
                            ugli::Condition::NotEqual
                        } else {
                            ugli::Condition::Equal
                        },
                        reference: 1,
                        mask: 0xff,
                    },
                    op: ugli::StencilOp::always(ugli::StencilOpFunc::Keep),
                })),
                blend_mode: Some(ugli::BlendMode::straight_alpha()),
                ..default()
            },
        );
    }
}

impl geng::State for Game {
    fn update(&mut self, delta_time: f64) {
        let delta_time = delta_time as f32;

        self.round_countdown -= delta_time;

        self.drift_particles.update(delta_time);
        self.bounce_particles.update(delta_time);
        self.t += delta_time;

        if self.music_on != self.music.is_some() {
            preferences::save("music_on", &self.music_on);
        }
        if !self.music_on {
            if let Some(mut music) = self.music.take() {
                music.stop();
            }
        } else {
            if self.music_menu != self.in_settings {
                if let Some(mut music) = self.music.take() {
                    music.stop();
                }
                self.music_menu = self.in_settings;
            }
            if self.music.is_none() {
                let mut music = match self.music_menu {
                    false => &self.assets.music,
                    true => &self.assets.menu_music,
                }
                .effect();
                music.set_volume(self.config.music_volume as f64);
                music.play();
                self.music = Some(music);
            }
        }

        self.cat_move_time -= delta_time;

        self.update_connection();
        for player in self.remote_players.values_mut() {
            player.update(delta_time);
        }

        self.update_my_player(delta_time);

        if let Some((_text, time)) = &mut self.text {
            *time += delta_time;
            if *time > 1.0 {
                self.text = None;
            }
        }
        if let Some((_text, time)) = &mut self.text2 {
            *time += delta_time;
            if *time > 1.0 {
                self.text2 = None;
            }
        }
    }
    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        self.framebuffer_size = framebuffer.size().map(|x| x as f32);

        if self.in_settings {
            let mut texture = ugli::Texture::new_uninitialized(
                self.geng.ugli(),
                framebuffer.size().map(|x| (x / 20).max(1)),
            );
            // texture.set_filter(ugli::Filter::Nearest);
            {
                let framebuffer = &mut ugli::Framebuffer::new_color(
                    self.geng.ugli(),
                    ugli::ColorAttachment::Texture(&mut texture),
                );
                self.draw_game(framebuffer);
            }
            self.geng.draw_2d(
                framebuffer,
                &geng::PixelPerfectCamera,
                &draw_2d::TexturedQuad::colored(
                    Aabb2::point(vec2::ZERO).extend_positive(self.framebuffer_size),
                    texture,
                    Rgba::new(0.3, 0.3, 0.3, 1.0),
                ),
            );
        } else {
            self.draw_game(framebuffer);
        }
    }

    fn handle_event(&mut self, event: geng::Event) {
        match event {
            geng::Event::Wheel { delta } => {
                if self.spectating {
                    self.spectate_zoomed_in = delta > 0.0;
                }
            }
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
                if self.geng.window().is_key_pressed(geng::Key::LCtrl) && self.args.editor =>
            {
                self.level.save(run_dir().join("level.json"));
            }
            geng::Event::KeyDown { key: geng::Key::E } if self.args.editor => {
                let pos = self.camera.screen_to_world(
                    self.framebuffer_size,
                    self.geng.window().mouse_pos().map(|x| x as f32),
                );
                self.level.cat_locations.push(pos);
            }
            geng::Event::KeyDown {
                key: geng::Key::Delete,
            } if self.args.editor => {
                let pos = self.camera.screen_to_world(
                    self.framebuffer_size,
                    self.geng.window().mouse_pos().map(|x| x as f32),
                );
                self.level
                    .cat_locations
                    .retain(|&p| (p - pos).len() > SNAP_DISTANCE);
            }
            geng::Event::KeyDown { key: geng::Key::T } if self.args.editor => {
                if let Some(player) = &mut self.player {
                    player.pos = self.camera.screen_to_world(
                        self.framebuffer_size,
                        self.geng.window().mouse_pos().map(|x| x as f32),
                    );
                }
            }
            geng::Event::KeyDown { key: geng::Key::M } if !self.in_settings => {
                self.music_on = !self.music_on; // TODO ui
            }
            geng::Event::KeyDown { key } if self.in_settings => {
                let old_name = self.name.clone();
                if key == geng::Key::Backspace {
                    self.name.pop();
                } else if self.name.len() < self.config.max_name_len {
                    let c = format!("{key:?}");
                    if c.len() == 1 {
                        self.name.push_str(&c);
                    }
                }
                if self.name != old_name {
                    preferences::save("name", &self.name);
                }
            }
            _ => {}
        }
    }

    fn ui<'a>(&'a mut self, cx: &'a geng::ui::Controller) -> Box<dyn geng::ui::Widget + 'a> {
        use geng::ui::*;
        let padding = 0.5;
        let settings_button =
            TextureButton::new(cx, &self.assets.ui.settings, &self.assets.ui.sfx, 1.0);
        if settings_button.was_clicked() {
            self.in_settings = true;
            self.ready = false;
            if self.practice.is_some() {
                self.player = None;
                self.spectating = true;
            }
            self.connection.send(ClientMessage::Ready(false));
        }

        let telecam_checkbox = TextureButton::new(
            cx,
            if self.telecam {
                &self.assets.ui.telecam_on
            } else {
                &self.assets.ui.telecam_off
            },
            &self.assets.ui.sfx,
            1.0,
        );
        if telecam_checkbox.was_clicked() {
            self.telecam = !self.telecam;
        }
        let telecam_checkbox = telecam_checkbox
            .fixed_size(vec2(2.0, 1.0))
            .uniform_padding(padding)
            .align(vec2(0.0, 0.0));
        let settings_button = settings_button
            .uniform_padding(padding)
            .align(vec2(1.0, 0.0));
        if self.in_settings {
            let gray = Rgba::new(0.8, 0.8, 0.8, 1.0);
            let checkbox_with_callback = |value: &mut bool, f: &dyn Fn(bool)| {
                let button = TextureButton::new(
                    cx,
                    if *value {
                        &self.assets.ui.names_check
                    } else {
                        &self.assets.ui.names_uncheck
                    },
                    &self.assets.ui.sfx,
                    1.0,
                );
                if button.was_clicked() {
                    *value = !*value;
                    f(*value);
                }
                button
            };
            let checkbox = |value: &mut bool| {
                let button = TextureButton::new(
                    cx,
                    if *value {
                        &self.assets.ui.music_check
                    } else {
                        &self.assets.ui.music_uncheck
                    },
                    &self.assets.ui.sfx,
                    1.0,
                );
                if button.was_clicked() {
                    *value = !*value;
                }
                button
            };

            let play_button = TextureButton::new2(
                cx,
                &self.assets.ui.play_unhovered,
                &self.assets.ui.play,
                &self.assets.ui.sfx,
                1.0,
            );

            let practice_button = TextureButton::new2(
                cx,
                &self.assets.ui.practice_unhovered,
                &self.assets.ui.practice,
                &self.assets.ui.sfx,
                1.0,
            );
            if play_button.was_clicked() {
                self.in_settings = false;
                self.ready = true;
                self.practice = None;
                self.connection.send(ClientMessage::Name(self.name.clone()));
                self.connection.send(ClientMessage::Ready(true));
            }
            if practice_button.was_clicked() {
                self.in_settings = false;
                self.ready = false;
                self.practice = Some(thread_rng().gen_range(0..self.level.cat_locations.len()));
                self.spectating = false;
                if self.player.is_none() {
                    self.player = Some(Player {
                        color: self.color,
                        skin: self.skin,
                        pos: vec2::ZERO,
                        vel: vec2::ZERO,
                        rot: thread_rng().gen_range(0.0..2.0 * f32::PI),
                    });
                }
                self.connection.send(ClientMessage::Ready(false));
            }
            let play_button = play_button
                .fixed_size(vec2(2.0, 1.0) * 1.5)
                .padding_top(-padding);
            let practice_button = practice_button
                .fixed_size(vec2(2.0, 1.0) * 1.5)
                .padding_top(-padding);

            let skin_button_previous =
                TextureButton::new(cx, &self.assets.ui.left, &self.assets.ui.sfx, 1.0);
            if skin_button_previous.was_clicked() {
                self.skin = (self.skin + self.assets.player.len() - 1) % self.assets.player.len();
                preferences::save("skin", &self.skin);
            }
            let skin_button_next =
                TextureButton::new(cx, &self.assets.ui.right, &self.assets.ui.sfx, 1.0);
            if skin_button_next.was_clicked() {
                self.skin = (self.skin + 1) % self.assets.player.len();
                preferences::save("skin", &self.skin);
            }
            if let Some(player) = &mut self.player {
                player.skin = self.skin;
                player.color = self.color;
            }
            let current_skin = stack![
                CarWidget::new(
                    &self.assets.car,
                    &self.assets.car_color,
                    batbox::color::Hsva::new(self.color, 1.0, 1.0, 1.0).into(),
                    2.0
                )
                .center(),
                TextureWidget::new(&self.assets.player[self.skin], 1.0).center()
            ];
            let customization = (
                (
                    CustomText::new(
                        if self.name.is_empty() {
                            "just type your name"
                        } else {
                            self.name.as_str()
                        },
                        &self.assets.font,
                        1.0,
                        if self.name.is_empty() {
                            gray
                        } else {
                            Rgba::WHITE
                        },
                    )
                    .center()
                    .fixed_size(vec2(5.0, 1.0))
                    .center(),
                    // Text::new("show player names", &self.assets.font, 1.0, gray).center(),
                    checkbox_with_callback(&mut self.show_player_names, &|value| {
                        preferences::save("show_player_names", &value);
                    })
                    .padding_left(padding)
                    .center(),
                )
                    .row()
                    .center(),
                (
                    skin_button_previous.center(),
                    current_skin.center(),
                    skin_button_next.center(),
                )
                    .row()
                    .center(),
                (
                    TextureWidget::new(&self.assets.ui.color, 1.0)
                        .padding_right(padding)
                        .center(),
                    CustomSlider::new(
                        cx,
                        &self.assets,
                        &self.assets.ui.slider_line,
                        &self.assets.ui.slider_knob,
                        self.color as f64,
                        0.0..=1.0,
                        Box::new(|new_value| {
                            self.color = new_value as f32;
                            preferences::save("color", &self.color);
                        }),
                    )
                    .fixed_size({
                        let mut size = self.assets.ui.slider_line.size().map(|x| x as f64);
                        size /= size.y;
                        size
                    })
                    .center(),
                )
                    .row()
                    .padding_top(padding)
                    .center(),
            )
                .column();
            let volume_settings = (
                // TextureWidget::new(&self.assets.ui.music, 1.0)
                //     .padding_right(padding)
                //     .center(),
                checkbox(&mut self.music_on)
                    .fixed_size(vec2(104.0 / 66.0, 1.0))
                    .center(),
                TextureWidget::new(&self.assets.ui.volume, 1.0)
                    .padding_left(padding)
                    .center(),
                CustomSlider::new(
                    cx,
                    &self.assets,
                    &self.assets.ui.volume_slider_line,
                    &self.assets.ui.slider_knob,
                    self.volume,
                    0.0..=1.0,
                    Box::new(|new_value| {
                        self.volume = new_value;
                        preferences::save("volume", &self.volume);
                        self.geng.audio().set_volume(new_value);
                    }),
                )
                .fixed_size({
                    let mut size = self.assets.ui.volume_slider_line.size().map(|x| x as f64);
                    size /= size.y;
                    size
                })
                .padding_left(padding)
                .center(),
            )
                .row();

            let game_title =
                TextureWidget::new(&self.assets.ui.title, 1.0).fixed_size(vec2(4.0, 2.0) * 2.0);
            let instructions = TextureWidget::new(&self.assets.ui.instructions, 1.0)
                .fixed_size(vec2(4.0, 2.0) * 2.0);

            let settings = (
                game_title.center(),
                (play_button.center(), practice_button.center())
                    .row()
                    .center(),
                (
                    TextureWidget::new(&self.assets.ui.background, 1.0),
                    (
                        (instructions.center(), volume_settings.center())
                            .column()
                            .center(),
                        customization
                            .center()
                            .padding_left(padding)
                            .padding_top(padding),
                    )
                        .row()
                        .uniform_padding(padding * 2.0)
                        .center(),
                )
                    .stack()
                    .center(),
            )
                .column()
                .center();
            settings.boxed()
        } else if self.spectating {
            stack![settings_button, telecam_checkbox].boxed()
        } else {
            settings_button.boxed()
        }
    }
}
