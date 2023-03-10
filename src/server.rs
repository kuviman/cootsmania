use super::*;

struct Client {
    ready: bool,
    name: String,
    pos: Option<vec2<f32>>,
    current_replay: bots::MoveData,
    sender: Box<dyn geng::net::Sender<ServerMessage>>,
}

struct Bot {
    index: usize,
}

struct State {
    round_countdown: Option<Timer>,
    next_id: Id,
    level: Level,
    config: Config,
    bots: bots::Data,
    clients: HashMap<Id, Client>,
    round: Round,
    qualified_players: HashSet<Id>,
    bot_ids: HashMap<Id, Bot>,
    players: HashSet<Id>,
    round_timer: Option<Timer>,
    new_session_timer: Option<Timer>,
    numbers: Numbers,
}

impl State {
    const TICKS_PER_SECOND: f32 = 10.0;
    fn new() -> Self {
        let level: Level =
            serde_json::from_reader(std::fs::File::open(run_dir().join("level.json")).unwrap())
                .unwrap();
        let config: Config =
            serde_json::from_reader(std::fs::File::open(run_dir().join("config.json")).unwrap())
                .unwrap();
        let mut bots = futures::executor::block_on(bots::Data::load(run_dir().join("bots.data")));
        for (track, data) in &mut bots.0 {
            data.retain(|data| {
                (data.data.first().unwrap().data.pos - level.cat_locations[track.from]).len()
                    < config.player_radius * 2.0
                    && (data.data.last().unwrap().data.pos - level.cat_locations[track.to]).len()
                        < config.player_radius * 2.0
            });
            data.sort_by_key(|data| r32(-data.data.last().unwrap().time));
        }

        let mut next_id = 0;
        let bot_ids = (0..config.min_players)
            .map(|index| {
                let data = (next_id, Bot { index });
                next_id += 1;
                data
            })
            .collect();
        Self {
            round_countdown: None,
            level,
            config,
            bots,
            next_id,
            clients: default(),
            round: Round {
                track: Track { from: 0, to: 1 },
                to_be_qualified: 1,
                num: 0,
            },
            qualified_players: default(),
            players: default(),
            bot_ids,
            round_timer: None,
            new_session_timer: None,
            numbers: Numbers {
                players_left: 0,
                spectators: 0,
                bots: 0,
                qualified: 0,
            },
        }
    }
    fn tick(&mut self) {
        self.update_numbers();
        if let Some(timer) = &mut self.round_countdown {
            if timer.elapsed().as_secs_f64() as f32 > 3.0 {
                let start_pos = self.level.cat_locations[self.round.track.from];
                for id in &self.players {
                    if let Some(client) = self.clients.get_mut(id) {
                        client.pos = Some(start_pos);
                        client
                            .sender
                            .send(ServerMessage::YouHaveBeenRespawned(start_pos));
                    }
                }
                for client in self.clients.values_mut() {
                    client.sender.send(ServerMessage::RoundStarted);
                }
                self.round_timer = Some(Timer::new());
                self.round_countdown = None;
                info!("Round started");
            }
            return;
        }
        if let Some(timer) = &mut self.new_session_timer {
            if timer.elapsed().as_secs_f64() as f32 > self.config.new_session_time {
                self.new_session_timer = None;
                self.new_session();
            }
            return;
        }

        if let Some(round_timer) = &self.round_timer {
            if round_timer.elapsed().as_secs_f64() > self.config.cat_move_time as f64
                || self.players.is_empty()
            {
                self.time_up();
            }
        }

        if let Some(round_timer) = &self.round_timer {
            let mut bots = self
                .bots
                .get(self.round.track, round_timer.elapsed().as_secs_f64() as f32);
            let mut bot_updates = Vec::new();
            let mut remove_bots = Vec::new();
            for &id in &self.players {
                if self.qualified_players.contains(&id) {
                    continue;
                }
                if let Some(bot) = self.bot_ids.get(&id) {
                    if let Some(player) = bots.next() {
                        bot_updates.push((id, player));
                    } else {
                        remove_bots.push(id);
                    }
                }
            }
            mem::drop(bots);
            if !remove_bots.is_empty() {
                for id in remove_bots {
                    self.players.remove(&id);
                }
            }
            for (id, player) in bot_updates {
                self.update_player(id, player);
            }
        }

        if self.qualified_players.len() >= self.round.to_be_qualified
            || self.players.len() == self.qualified_players.len()
        {
            self.end_round();
        }
    }
    fn new_session(&mut self) {
        info!("Starting new session");
        let start = thread_rng().gen_range(0..self.level.cat_locations.len());
        self.players = itertools::chain![
            self.clients
                .iter()
                .filter_map(|(&id, client)| client.ready.then_some(id)),
            self.bot_ids.keys().copied()
        ]
        .take(self.clients.len().max(self.config.min_players))
        .collect();
        if self.players.iter().all(|id| self.bot_ids.contains_key(id)) {
            self.players.clear();
        }
        self.new_round_from(0, start);
    }
    fn new_round_from(&mut self, num: usize, from: usize) {
        self.round = Round {
            num,
            track: self.level.random_track_from(from),
            to_be_qualified: if num == 0 {
                self.players.len()
            } else {
                self.players.len()
                    - ((self.players.len() as f32 * self.config.elimination_ratio) as usize)
                        .max(1)
                        .min(self.players.len())
            },
        };
        for client in self.clients.values_mut() {
            client
                .sender
                .send(ServerMessage::NewRound(self.round.clone()));
        }

        info!("About to start new round...");
        self.round_countdown = Some(Timer::new());
        self.qualified_players.clear();
    }
    fn player_finished(&mut self, id: Id) {
        if let Some(client) = self.clients.get_mut(&id) {
            client.pos = None;
            client.sender.send(ServerMessage::YouHaveBeenQualified);
        }
        for (&client_id, client) in &mut self.clients {
            if client_id != id {
                client.sender.send(ServerMessage::UpdatePlayer(id, None));
            }
        }
        self.qualified_players.insert(id);
    }
    fn time_up(&mut self) {
        self.end_round();
    }

    fn update_numbers(&mut self) {
        let players_left = self.players.len();
        let bots = self
            .players
            .iter()
            .filter(|id| self.bot_ids.contains_key(id))
            .count();
        let actual_players_left = players_left - bots;
        let spectators = self.clients.len() - actual_players_left;
        let qualified = self.qualified_players.len();
        self.numbers = Numbers {
            players_left,
            spectators,
            bots,
            qualified,
        };
    }

    fn end_round(&mut self) {
        self.round_timer = None;
        if self.config.server_recordings {
            for client in self.clients.values_mut() {
                let replay = mem::replace(&mut client.current_replay, bots::MoveData::new());
                self.bots.push(self.round.track, replay);
            }
            bincode::serialize_into(
                std::io::BufWriter::new(
                    std::fs::File::create(run_dir().join("bots.data")).unwrap(),
                ),
                &self.bots,
            )
            .unwrap();
        }

        for (&id, client) in &mut self.clients {
            if !self.qualified_players.contains(&id) && client.pos.is_some() {
                client.pos = None;
                client.sender.send(ServerMessage::YouHaveBeenEliminated);
            }
        }

        self.players
            .retain(|id| self.qualified_players.contains(id));
        if self.players.len() <= 1 {
            let winner = self.players.iter().copied().next();
            for (&client_id, client) in &mut self.clients {
                if Some(client_id) == winner {
                    client.pos = Some(self.level.cat_locations[self.round.track.to]);
                    client.sender.send(ServerMessage::YouHaveBeenRespawned(
                        self.level.cat_locations[self.round.track.to],
                    ));
                    client.sender.send(ServerMessage::YouAreWinner);
                } else {
                    client.sender.send(ServerMessage::Winner(winner));
                }
            }
            if let Some(winner) = winner {
                if self.bot_ids.contains_key(&winner) {
                    for client in self.clients.values_mut() {
                        client.sender.send(ServerMessage::UpdatePlayer(
                            winner,
                            Some(Player {
                                color: 0.0,
                                skin: 0,
                                pos: self.level.cat_locations[self.round.track.to],
                                vel: vec2::ZERO,
                                rot: 0.0,
                            }),
                        ));
                    }
                }
            }
            self.new_session_timer = Some(Timer::new());
        } else {
            self.new_round_from(self.round.num + 1, self.round.track.to);
        }
    }

    fn update_player(&mut self, id: Id, player: Player) {
        if let Some(client) = self.clients.get(&id) {
            if client.pos.is_none() {
                // Ignore, is you cheating???
                return;
            }
        }

        let message = Arc::new(geng::net::serialize_message(ServerMessage::UpdatePlayer(
            id,
            Some(player.clone()),
        )));
        for (&client_id, client) in &mut self.clients {
            if client_id == id {
                client.pos = Some(player.pos);
                if self.config.server_recordings {
                    if let Some(round_timer) = &self.round_timer {
                        if !self.qualified_players.contains(&id) {
                            client
                                .current_replay
                                .push(round_timer.elapsed().as_secs_f64() as f32, player.clone());
                        }
                    }
                }
            } else {
                client.sender.send_serialized(message.clone());
            }
        }

        self.check_finished(id, player);
    }

    fn check_finished(&mut self, id: Id, player: Player) {
        if self.qualified_players.contains(&id) {
            return;
        }
        if player.vel.len() > 1e-5 {
            return;
        }
        if (player.pos - self.level.cat_locations[self.round.track.to]).len()
            > self.config.player_radius * 2.0
        {
            return;
        }
        self.player_finished(id);
    }
}

pub struct App {
    state: Arc<Mutex<State>>,
    #[allow(dead_code)]
    background_thread: std::thread::JoinHandle<()>,
}

impl App {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(State::new()));
        Self {
            state: state.clone(),
            background_thread: std::thread::spawn(move || loop {
                state.lock().unwrap().tick();
                std::thread::sleep(std::time::Duration::from_secs_f32(
                    1.0 / State::TICKS_PER_SECOND,
                ));
            }),
        }
    }
}

pub struct ClientConnection {
    id: Id,
    state: Arc<Mutex<State>>,
}

impl Drop for ClientConnection {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.clients.remove(&self.id);
        state.players.remove(&self.id);
        for other in state.clients.values_mut() {
            other.sender.send(ServerMessage::Disconnect(self.id));
        }
    }
}

fn fix(player: &mut Player) {
    if !player.pos.x.is_finite() {
        player.pos.x = 0.0;
    }
    if !player.pos.y.is_finite() {
        player.pos.y = 0.0;
    }
    if !player.vel.x.is_finite() {
        player.vel.x = 0.0;
    }
    if !player.vel.y.is_finite() {
        player.vel.y = 0.0;
    }
    if !player.rot.is_finite() {
        player.rot = 0.0;
    }
}

impl geng::net::Receiver<ClientMessage> for ClientConnection {
    fn handle(&mut self, message: ClientMessage) {
        let mut state = self.state.lock().unwrap();
        let state: &mut State = state.deref_mut();
        match message {
            ClientMessage::Ready(ready) => {
                state
                    .clients
                    .get_mut(&self.id)
                    .expect("Sender not found for client")
                    .ready = ready;
            }
            ClientMessage::Ping => {
                state
                    .clients
                    .get_mut(&self.id)
                    .expect("Sender not found for client")
                    .sender
                    .send(ServerMessage::Pong);
                state
                    .clients
                    .get_mut(&self.id)
                    .unwrap()
                    .sender
                    .send(ServerMessage::Numbers(state.numbers.clone()));
            }
            ClientMessage::UpdatePlayer(mut player) => {
                fix(&mut player);
                state.update_player(self.id, player);
            }
            ClientMessage::Name(name) => {
                let name = name.chars().filter(|c| c.is_ascii_alphabetic()).take(15);
                let name: String = rustrict::CensorIter::censor(name).collect();

                state.clients.get_mut(&self.id).unwrap().name = name.clone();
                for (&client_id, client) in &mut state.clients {
                    if self.id == client_id {
                        client.sender.send(ServerMessage::YourName(name.clone()));
                    } else {
                        client
                            .sender
                            .send(ServerMessage::Name(self.id, name.clone()));
                    }
                }
            }
        }
    }
}

impl geng::net::server::App for App {
    type Client = ClientConnection;
    type ServerMessage = ServerMessage;
    type ClientMessage = ClientMessage;
    fn connect(
        &mut self,
        mut sender: Box<dyn geng::net::Sender<Self::ServerMessage>>,
    ) -> ClientConnection {
        let mut state = self.state.lock().unwrap();
        for (&id, client) in &state.clients {
            sender.send(ServerMessage::Name(id, client.name.clone()));
        }
        let id = state.next_id;
        state.clients.insert(
            id,
            Client {
                ready: false,
                name: String::new(),
                current_replay: bots::MoveData::new(),
                pos: None,
                sender,
            },
        );
        state.next_id += 1;
        ClientConnection {
            id,
            state: self.state.clone(),
        }
    }
}

#[test]
fn test_brainoid() {
    assert_eq!(
        "brainoid",
        rustrict::CensorIter::censor("brainoid".chars()).collect::<String>()
    );
}
