use super::*;

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
pub struct Assets {}

pub struct PlayerInput {
    rotate: f32,     // -1 .. 1
    accelerate: f32, // -1 .. 1
}

type Connection = geng::net::client::Connection<ServerMessage, ClientMessage>;

struct RemotePlayer {
    pos: Interpolated<vec2<f32>>,
    rot: f32,
}

impl RemotePlayer {
    fn new(player: Player) -> Self {
        Self {
            pos: Interpolated::new(player.pos, player.vel),
            rot: player.rot,
        }
    }
    fn server_update(&mut self, upd: Player) {
        self.pos.server_update(upd.pos, upd.vel);
        self.rot = upd.rot;
    }
    fn update(&mut self, delta_time: f32) {
        self.pos.update(delta_time);
    }

    fn get(&self) -> Player {
        Player {
            pos: self.pos.get(),
            vel: self.pos.get_derivative(),
            rot: self.rot,
        }
    }
}

pub struct Game {
    geng: Geng,
    assets: Rc<Assets>,
    config: Rc<Config>,
    connection: Connection,
    player: Option<Player>,
    camera: geng::Camera2d,
    level: Level,
    args: Args,
    start_drag: Option<vec2<f32>>,
    framebuffer_size: vec2<f32>,
    remote_players: HashMap<Id, RemotePlayer>,
    cat_location: Option<usize>,
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
        Self {
            geng: geng.clone(),
            assets: assets.clone(),
            level,
            connection,
            config: config.clone(),
            player: None,
            camera: geng::Camera2d {
                center: vec2::ZERO,
                rotation: 0.0,
                fov: 50.0,
            },
            args,
            start_drag: None,
            framebuffer_size: vec2(1.0, 1.0),
            remote_players: default(),
            cat_location: None,
        }
    }

    fn update_connection(&mut self) {
        while let Some(message) = self.connection.try_recv() {
            match message {
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
                ServerMessage::UpdateCat(index) => {
                    self.cat_location = index;
                }
                ServerMessage::YouHaveBeenEliminated => {
                    self.player = None;
                }
                ServerMessage::YouHaveBeenRespawned(pos) => {
                    self.player = Some(Player {
                        pos,
                        vel: vec2::ZERO,
                        rot: thread_rng().gen_range(0.0..2.0 * f32::PI),
                    });
                }
            }
        }
    }

    fn draw_player(
        &self,
        framebuffer: &mut ugli::Framebuffer,
        camera: &geng::Camera2d,
        player: &Player,
        me: bool,
    ) {
        self.geng.draw_2d(
            framebuffer,
            camera,
            &draw_2d::Ellipse::circle(
                player.pos,
                self.config.player_radius,
                if me { Rgba::GREEN } else { Rgba::GRAY },
            ),
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
    }
}

impl geng::State for Game {
    fn update(&mut self, delta_time: f64) {
        let delta_time = delta_time as f32;

        self.update_connection();
        for player in self.remote_players.values_mut() {
            player.update(delta_time);
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

        if let Some(player) = &mut self.player {
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

            self.camera.center += (player.pos - self.camera.center)
                * (self.config.camera_speed * delta_time).min(1.0);
        }
    }
    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        self.framebuffer_size = framebuffer.size().map(|x| x as f32);
        ugli::clear(framebuffer, Some(Rgba::BLACK), None, None);

        let camera = &self.camera;
        for player in self.remote_players.values() {
            self.draw_player(framebuffer, camera, &player.get(), false);
        }
        if let Some(index) = self.cat_location {
            self.geng.draw_2d(
                framebuffer,
                camera,
                &draw_2d::Ellipse::circle(self.level.cat_locations[index], 1.0, Rgba::WHITE),
            );
        }
        if let Some(player) = &self.player {
            self.draw_player(framebuffer, camera, player, true);
        }
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
            for &p in &self.level.cat_locations {
                self.geng.draw_2d(
                    framebuffer,
                    camera,
                    &draw_2d::Quad::new(Aabb2::point(p).extend_uniform(0.3), Rgba::GREEN),
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
            _ => {}
        }
    }
}
