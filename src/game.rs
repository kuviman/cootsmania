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
pub struct UiAssets {
    left: ugli::Texture,
    right: ugli::Texture,
}

#[derive(geng::Assets)]
pub struct Assets {
    map_floor: ugli::Texture,
    map_furniture: ugli::Texture,
    coots: ugli::Texture,
    arrow: ugli::Texture,
    #[asset(load_with = "load_player_assets(&geng, base_path.join(\"player\"))")]
    player: Vec<ugli::Texture>,
    player_direction: ugli::Texture,
    ui: UiAssets,
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
    pos: Interpolated<vec2<f32>>,
    rot: f32,
}

impl RemotePlayer {
    fn new(player: Player) -> Self {
        Self {
            skin: player.skin,
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
            skin: self.skin,
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
    cat_move_time: f32,
    text: Option<(String, f32)>,
    score: i32,
    placement: usize,
    skin: usize,
    in_settings: bool,
    volume: f64,
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
        let volume = 0.5;
        geng.audio().set_volume(volume);
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
                fov: config.camera_fov,
            },
            args,
            start_drag: None,
            framebuffer_size: vec2(1.0, 1.0),
            remote_players: default(),
            cat_location: None,
            cat_move_time: 0.0,
            text: None,
            score: 0,
            placement: 0,
            skin: thread_rng().gen_range(0..assets.player.len()),
            in_settings: true,
            volume,
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
                ServerMessage::UpdateCat {
                    location,
                    move_time,
                } => {
                    self.cat_location = location;
                    self.cat_move_time = move_time;
                }
                ServerMessage::YouHaveBeenEliminated => {
                    self.player = None;
                    self.text = Some(("You have been eliminated".to_owned(), 0.0));
                }
                ServerMessage::YouHaveBeenRespawned(pos) => {
                    self.score = 0;
                    self.placement = 0;
                    self.player = Some(Player {
                        skin: self.skin,
                        pos,
                        vel: vec2::ZERO,
                        rot: thread_rng().gen_range(0.0..2.0 * f32::PI),
                    });
                    self.text = Some(("New game! Go to coots now!".to_owned(), 0.0));
                }
                ServerMessage::YouScored(score) => {
                    self.score += score;
                    self.text = Some((format!("+{score}"), 0.0));
                }
                ServerMessage::UpdatePlacement(placment) => {
                    self.placement = placment;
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
        if let Some(texture) = self.assets.player.get(player.skin) {
            self.geng.draw_2d(
                framebuffer,
                camera,
                &draw_2d::TexturedQuad::unit_colored(
                    texture,
                    if me {
                        Rgba::WHITE
                    } else {
                        Rgba::new(1.0, 1.0, 1.0, 0.5)
                    },
                )
                .scale_uniform(self.config.player_radius)
                .translate(player.pos),
            );
            if me {
                self.geng.draw_2d(
                    framebuffer,
                    camera,
                    &draw_2d::TexturedQuad::unit(&self.assets.player_direction)
                        .rotate(player.rot)
                        .scale(self.config.player_direction_scale * self.config.player_radius)
                        .translate(player.pos),
                );
            }
        }
    }

    fn update_my_player(&mut self, delta_time: f32) {
        let Some(player) = &mut self.player else { return };

        self.camera.center +=
            (player.pos - self.camera.center) * (self.config.camera_speed * delta_time).min(1.0);

        if player.vel.len() < 1e-5 {
            if let Some(index) = self.cat_location {
                if let Some(&pos) = self.level.cat_locations.get(index) {
                    if (player.pos - pos).len() < self.config.player_radius * 2.0 {
                        return;
                    }
                }
            }
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

    fn draw_game(&self, framebuffer: &mut ugli::Framebuffer) {
        ugli::clear(framebuffer, Some(Rgba::BLACK), None, None);

        let camera = &self.camera;
        let camera_aabb = camera.view_area(self.framebuffer_size).bounding_box();

        let texture_pos = Aabb2::point(vec2::ZERO).extend_symmetric({
            let size = self.assets.map_floor.size().map(|x| x as f32);
            vec2(size.x / size.y, 1.0) * 10.0
        });
        self.geng.draw_2d(
            framebuffer,
            camera,
            &draw_2d::TexturedQuad::new(texture_pos, &self.assets.map_floor),
        );

        for player in self.remote_players.values() {
            self.draw_player(framebuffer, camera, &player.get(), false);
        }
        if let Some(index) = self.cat_location {
            if let Some(&pos) = self.level.cat_locations.get(index) {
                self.geng.draw_2d(
                    framebuffer,
                    camera,
                    &draw_2d::TexturedQuad::new(
                        Aabb2::point(pos).extend_uniform(self.config.player_radius),
                        &self.assets.coots,
                    ),
                );
                if !camera_aabb.contains(pos) {
                    let aabb = camera_aabb.extend_uniform(-self.config.arrow_size);
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
                }
            } else {
                error!("Cat location not found!");
            }
        }
        if let Some(player) = &self.player {
            self.draw_player(framebuffer, camera, player, true);
        }

        self.geng.draw_2d(
            framebuffer,
            camera,
            &draw_2d::TexturedQuad::new(texture_pos, &self.assets.map_furniture),
        );

        let ui_camera = &geng::Camera2d {
            center: vec2::ZERO,
            rotation: 0.0,
            fov: 10.0,
        };
        let ui_aabb = ui_camera.view_area(self.framebuffer_size).bounding_box();
        self.geng.default_font().draw(
            framebuffer,
            ui_camera,
            &format!(
                "coots moves in {}s",
                self.cat_move_time.max(0.0).ceil() as i64,
            ),
            vec2(0.0, 4.0),
            geng::TextAlign::CENTER,
            1.0,
            Rgba::GRAY,
        );
        self.geng.default_font().draw(
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
            Rgba::GRAY,
        );
        if self.placement != 0 {
            self.geng.default_font().draw_with_outline(
                framebuffer,
                ui_camera,
                &format!("#{}", self.placement),
                ui_aabb.bottom_right() + vec2(-1.0, 2.0),
                geng::TextAlign::RIGHT,
                1.0,
                Rgba::WHITE,
                0.05,
                Rgba::BLACK,
            );
        }
        self.geng.default_font().draw_with_outline(
            framebuffer,
            ui_camera,
            &format!("score: {}", self.score),
            ui_aabb.bottom_right() + vec2(-1.0, 1.0),
            geng::TextAlign::RIGHT,
            0.7,
            Rgba::WHITE,
            0.01,
            Rgba::BLACK,
        );
        if let Some((ref text, t)) = self.text {
            self.geng.default_font().draw_with_outline(
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

        for &[p1, p2] in &self.level.segments {
            self.geng.draw_2d(
                framebuffer,
                camera,
                &draw_2d::Segment::new(Segment(p1, p2), 0.1, Rgba::WHITE),
            );
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
                    Rgba::GRAY,
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
            _ => {}
        }
    }

    fn ui<'a>(&'a mut self, cx: &'a geng::ui::Controller) -> Box<dyn geng::ui::Widget + 'a> {
        use geng::ui::*;
        let settings_button = Button::new(cx, "settings");
        if settings_button.was_clicked() {
            self.in_settings = !self.in_settings;
        }
        let settings_button = settings_button.uniform_padding(16.0).align(vec2(0.0, 1.0));
        if self.in_settings {
            let skin_button_previous = TextureButton::new(cx, &self.assets.ui.left, 32.0);
            if skin_button_previous.was_clicked() {
                self.skin = (self.skin + self.assets.player.len() - 1) % self.assets.player.len();
            }
            let skin_button_next = TextureButton::new(cx, &self.assets.ui.right, 32.0);
            if skin_button_next.was_clicked() {
                self.skin = (self.skin + 1) % self.assets.player.len();
            }
            if let Some(player) = &mut self.player {
                player.skin = self.skin;
            }
            let current_skin = TextureWidget::new(&self.assets.player[self.skin], 32.0);
            let skin_settings = (
                skin_button_previous.center(),
                current_skin.center(),
                skin_button_next.center(),
            )
                .row();
            let volume_settings = (
                "volume".center(),
                Slider::new(
                    cx,
                    self.volume,
                    0.0..=1.0,
                    Box::new(|new_value| {
                        self.volume = new_value;
                        self.geng.audio().set_volume(new_value);
                    }),
                )
                .fixed_size(vec2(100.0, cx.theme().text_size as f64))
                .center(),
            )
                .row();
            let settings = (skin_settings.center(), volume_settings.center())
                .column()
                .center();
            stack![settings_button, settings].boxed()
        } else {
            settings_button.boxed()
        }
    }
}
