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
    #[asset(ext = "mp3", postprocess = "make_looped")]
    pub forward_move: geng::Sound,
}

#[derive(geng::Assets)]
pub struct UiAssets {
    color: ugli::Texture,
    title: ugli::Texture,
    instructions: ugli::Texture,
    play: ugli::Texture,
    left: ugli::Texture,
    right: ugli::Texture,
    settings: ugli::Texture,
    volume: ugli::Texture,
    slider_line: ugli::Texture,
    slider_knob: ugli::Texture,
    bots: ugli::Texture,
    spectators: ugli::Texture,
    players_left: ugli::Texture,
    time: ugli::Texture,
    checkbox_on: ugli::Texture,
    checkbox_off: ugli::Texture,
    music: ugli::Texture,
}

#[derive(geng::Assets)]
pub struct Assets {
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
    ui: UiAssets,
    #[asset(
        range = r#"["Slow", "Medium", "Fast"]"#,
        path = "music/Ludwig23_*.mp3",
        postprocess = "make_each_looped"
    )]
    music: Vec<geng::Sound>,
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
    fn draw(&self, framebuffer: &mut ugli::Framebuffer, camera: &geng::Camera2d) {
        // TODO optimize
        for particle in &self.data {
            self.geng.draw_2d(
                framebuffer,
                camera,
                &draw_2d::TexturedQuad::unit_colored(
                    &self.assets.particle,
                    Rgba::new(
                        1.0,
                        1.0,
                        1.0,
                        (1.0 - particle.t) * self.config.particle_alpha,
                    ),
                )
                .scale_uniform(self.config.particle_size)
                .rotate(particle.rot)
                .translate(particle.pos),
            )
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Winner {
    None,
    Me,
    Other(Id),
}

pub struct Game {
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
    skin: usize,
    in_settings: bool,
    volume: f64,
    round: Round,
    numbers: Numbers,
    name: String,
    spectating: bool,
    music_on: bool,
    music: Option<geng::SoundEffect>,
    drift_sfx: geng::SoundEffect,
    forward_sfx: geng::SoundEffect,
    next_drift_particle: f32,
    particles: Particles,
    t: f32,
    show_player_names: bool,
}

impl Game {
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
            color,
            music_on,
            music: None,
            geng: geng.clone(),
            assets: assets.clone(),
            level,
            connection,
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
            particles: Particles::new(geng, config, assets),
            winner: None,
            t: 0.0,
            show_player_names: preferences::load("show_player_names").unwrap_or(true),
        }
    }

    fn update_connection(&mut self) {
        while let Some(message) = self.connection.try_recv() {
            match &message {
                ServerMessage::Pong => {}
                ServerMessage::UpdatePlayer(..) => {}
                _ => info!("{message:?}"),
            }
            match message {
                ServerMessage::Name(id, name) => {
                    self.names.insert(id, name);
                }
                ServerMessage::Pong => {
                    self.connection.send(ClientMessage::Ping);
                    if let Some(player) = &self.player {
                        self.connection
                            .send(ClientMessage::UpdatePlayer(player.clone()));
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
                    if !self.args.editor {
                        self.player = None;
                        self.text = Some(("You have been eliminated".to_owned(), 0.0));
                        self.spectating = true;
                        self.assets.sfx.eliminated.play();
                    }
                }
                ServerMessage::YouHaveBeenRespawned(pos) => {
                    if !self.args.editor {
                        self.player = Some(Player {
                            skin: self.skin,
                            color: self.color,
                            pos,
                            vel: vec2::ZERO,
                            rot: thread_rng().gen_range(0.0..2.0 * f32::PI),
                        });
                        self.spectating = false;
                    }
                }
                ServerMessage::Numbers(numbers) => {
                    self.numbers = numbers;
                }
                ServerMessage::NewRound(round) => {
                    self.winner = None;
                    self.cat_move_time = self.config.cat_move_time as f32;
                    self.remote_players.clear();
                    self.assets.sfx.new_round.play();
                    if round.num == 0 {
                        self.text = Some(("Warmup round! Go to coots now!".to_string(), -2.0));
                    } else {
                        self.text = Some((format!("Round {}! Go to coots now!", round.num), 0.0));
                    }
                    self.round = round;
                }
                ServerMessage::YouHaveBeenQualified => {
                    if !self.args.editor {
                        self.player = None;
                        self.text = Some(("QUALIFIED!!!".to_owned(), 0.0));
                        self.assets.sfx.qualified.play();
                    }
                }
                ServerMessage::YouAreWinner => {
                    self.winner = Some(Winner::Me);
                    self.text = Some(("You have won!".to_owned(), -2.0));
                }
                ServerMessage::Winner(winner) => match winner {
                    Some(id) => {
                        self.winner = Some(Winner::Other(id));
                        self.text = Some(("This is the winner!".to_owned(), -2.0));
                    }
                    None => {
                        self.winner = Some(Winner::None);
                        self.text = Some(("Nobody won!".to_owned(), -2.0));
                    }
                },
            }
        }
    }

    fn draw_player(
        &self,
        framebuffer: &mut ugli::Framebuffer,
        camera: &geng::Camera2d,
        player: &Player,
        id: Option<Id>, // None = me
    ) {
        let alpha = if id.is_some() { 0.5 } else { 1.0 };
        if let Some(texture) = self.assets.player.get(player.skin) {
            self.geng.draw_2d(
                framebuffer,
                camera,
                &draw_2d::TexturedQuad::unit_colored(
                    &self.assets.car,
                    Rgba::new(1.0, 1.0, 1.0, alpha),
                )
                .rotate(player.rot)
                .scale(self.config.player_direction_scale * self.config.player_radius)
                .translate(player.pos),
            );
            self.geng.draw_2d(
                framebuffer,
                camera,
                &draw_2d::TexturedQuad::unit_colored(
                    &self.assets.car_color,
                    batbox::color::Hsva::new(player.color, 1.0, 1.0, alpha).into(),
                )
                .rotate(player.rot)
                .scale(self.config.player_direction_scale * self.config.player_radius)
                .translate(player.pos),
            );
            self.geng.draw_2d(
                framebuffer,
                camera,
                &draw_2d::TexturedQuad::unit_colored(texture, Rgba::new(1.0, 1.0, 1.0, alpha))
                    .scale_uniform(self.config.player_radius)
                    .translate(player.pos + vec2(0.0, self.config.player_radius)),
            );

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
    }

    fn update_my_player(&mut self, delta_time: f32) {
        if self.spectating {
            if let Some(Winner::Other(id)) = self.winner {
                if let Some(p) = self.remote_players.get(&id) {
                    self.camera.center += (p.get().pos - self.camera.center)
                        * (self.config.camera_speed * delta_time).min(1.0);
                }
            } else {
                self.camera.center += (vec2::ZERO - self.camera.center)
                    * (self.config.camera_speed * delta_time).min(1.0);
            }
        }
        let player = match &mut self.player {
            Some(player) => player,
            None => {
                self.drift_sfx.set_volume(0.0);
                self.forward_sfx.set_volume(0.0);
                return;
            }
        };

        self.camera.center +=
            (player.pos - self.camera.center) * (self.config.camera_speed * delta_time).min(1.0);

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
        forward_vel +=
            (target_forward_vel - forward_vel).clamp_abs(forward_acceleration.abs() * delta_time);

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
            self.particles.push(player.pos, player.vel);
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
            }
            player.vel -= n * v * (1.0 + self.config.collision_bounciness);
        }
    }

    fn draw_game(&mut self, framebuffer: &mut ugli::Framebuffer) {
        ugli::clear(framebuffer, Some(Rgba::BLACK), None, None);

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

        if let Some(&pos) = self.level.cat_locations.get(self.round.track.to) {
            let mut pos = pos;
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

        for (&id, player) in &self.remote_players {
            self.draw_player(framebuffer, camera, &player.get(), Some(id));
        }
        self.particles.draw(framebuffer, camera);
        if let Some(player) = &self.player {
            self.draw_player(framebuffer, camera, player, None);
        }

        self.geng.draw_2d(
            framebuffer,
            camera,
            &draw_2d::TexturedQuad::new(texture_pos, &self.assets.map_furniture_front),
        );

        if let Some(&pos) = self.level.cat_locations.get(self.round.track.to) {
            let mut pos = pos;
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

        if let Some(&pos) = self.level.cat_locations.get(self.round.track.to) {
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

                self.geng.draw_2d(
                    framebuffer,
                    camera,
                    &draw_2d::TexturedQuad::unit(&self.assets.arrow)
                        .scale_uniform(self.config.arrow_size)
                        .rotate((pos - arrow_pos).arg())
                        .translate(arrow_pos),
                );
                self.geng.draw_2d(
                    framebuffer,
                    camera,
                    &draw_2d::TexturedQuad::unit(&self.assets.coots)
                        .scale_uniform(self.config.arrow_size * 0.5)
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
            if self.player.is_some() {
                "go to coots!"
            } else {
                "wait for current game to finish"
            },
            vec2(0.0, -4.0),
            geng::TextAlign::CENTER,
            1.0,
            Rgba::WHITE,
            0.05,
            Rgba::BLACK,
        );
        if let Some((ref text, t)) = self.text {
            self.assets.font.draw_with_outline(
                framebuffer,
                ui_camera,
                &text,
                vec2(0.0, 2.0),
                geng::TextAlign::CENTER,
                1.0,
                Rgba::WHITE,
                0.05,
                Rgba::BLACK,
            );
        }
        let padding = 0.2;
        let font_size = 0.5;
        let outline_size = 0.03;

        // Time
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
}

impl geng::State for Game {
    fn update(&mut self, delta_time: f64) {
        let delta_time = delta_time as f32;

        self.particles.update(delta_time);
        self.t += delta_time;

        if self.music_on != self.music.is_some() {
            preferences::save("music_on", &self.music_on);
        }
        if !self.music_on {
            if let Some(mut music) = self.music.take() {
                music.stop();
            }
        } else if self.music.is_none() {
            let mut music = self.assets.music[1].effect();
            music.set_volume(self.config.music_volume as f64);
            music.play();
            self.music = Some(music);
        }

        self.cat_move_time -= delta_time;

        let target_fov = if !self.spectating {
            self.config.camera_fov
        } else if let Some(Winner::Other(_)) = self.winner {
            self.config.camera_fov
        } else {
            self.config.map_scale * 2.0
        };
        self.camera.fov +=
            (target_fov - self.camera.fov).clamp_abs(self.config.zoom_speed * delta_time);

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
    }
    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        self.framebuffer_size = framebuffer.size().map(|x| x as f32);

        if self.in_settings {
            let mut texture = ugli::Texture::new_uninitialized(
                self.geng.ugli(),
                framebuffer.size().map(|x| (x / 20).max(1)),
            );
            texture.set_filter(ugli::Filter::Nearest);
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
                    self.connection.send(ClientMessage::Name(self.name.clone()));
                }
            }
            _ => {}
        }
    }

    fn ui<'a>(&'a mut self, cx: &'a geng::ui::Controller) -> Box<dyn geng::ui::Widget + 'a> {
        use geng::ui::*;
        let padding = 0.5;
        let settings_button = TextureButton::new(cx, &self.assets.ui.settings, 1.0);
        if settings_button.was_clicked() {
            self.in_settings = !self.in_settings;
        }
        let settings_button = settings_button
            .uniform_padding(padding)
            .align(vec2(1.0, 0.0));
        if self.in_settings {
            let gray = Rgba::new(0.8, 0.8, 0.8, 1.0);
            let checkbox_with_callback = |value: &mut bool, f: &dyn Fn(bool)| {
                let button = TextureButton::new(
                    cx,
                    if *value {
                        &self.assets.ui.checkbox_on
                    } else {
                        &self.assets.ui.checkbox_off
                    },
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
                        &self.assets.ui.checkbox_on
                    } else {
                        &self.assets.ui.checkbox_off
                    },
                    1.0,
                );
                if button.was_clicked() {
                    *value = !*value;
                }
                button
            };

            let play_button = TextureButton::new(cx, &self.assets.ui.play, 1.0);
            if play_button.was_clicked() {
                self.in_settings = false;
            }
            let play_button = play_button.fixed_size(vec2(2.0, 1.0));

            let skin_button_previous = TextureButton::new(cx, &self.assets.ui.left, 1.0);
            if skin_button_previous.was_clicked() {
                self.skin = (self.skin + self.assets.player.len() - 1) % self.assets.player.len();
                preferences::save("skin", &self.skin);
            }
            let skin_button_next = TextureButton::new(cx, &self.assets.ui.right, 1.0);
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
                Text::new(
                    if self.name.is_empty() {
                        "type your name by pressing keys"
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
                .center(),
                (
                    Text::new("show player names", &self.assets.font, 1.0, gray).center(),
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
                    TextureWidget::new(&self.assets.ui.color, 1.0)
                        .padding_left(padding * 3.0)
                        .padding_right(padding)
                        .center(),
                    CustomSlider::new(
                        cx,
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
                    .center(),
            )
                .column();
            let volume_settings = (
                TextureWidget::new(&self.assets.ui.volume, 1.0)
                    .uniform_padding(padding)
                    .center(),
                CustomSlider::new(
                    cx,
                    &self.assets.ui.slider_line,
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
                    let mut size = self.assets.ui.slider_line.size().map(|x| x as f64);
                    size /= size.y;
                    size
                })
                .center(),
                TextureWidget::new(&self.assets.ui.music, 1.0)
                    .uniform_padding(padding)
                    .center(),
                checkbox(&mut self.music_on).center(),
            )
                .row();

            let game_title =
                TextureWidget::new(&self.assets.ui.title, 1.0).fixed_size(vec2(4.0, 2.0));
            let instructions =
                TextureWidget::new(&self.assets.ui.instructions, 1.0).fixed_size(vec2(4.0, 2.0));

            let settings = (
                game_title.center(),
                (instructions.center(), play_button.center()).row().center(),
                customization.center(),
                volume_settings.center(),
            )
                .column()
                .center();
            settings.boxed()
        } else {
            settings_button.boxed()
        }
    }
}
