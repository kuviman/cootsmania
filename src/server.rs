use super::*;

struct Client {
    pos: Option<vec2<f32>>,
    current_replay: bots::MoveData,
    sender: Box<dyn geng::net::Sender<ServerMessage>>,
}

struct Bot {
    index: usize,
}

struct State {
    next_id: Id,
    level: Level,
    config: Config,
    bots: bots::Data,
    clients: HashMap<Id, Client>,
    round: Round,
    qualified_players: HashSet<Id>,
    bot_ids: HashMap<Id, Bot>,
    players: HashSet<Id>,
    round_timer: Timer,
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
        let bots = futures::executor::block_on(bots::Data::load(run_dir().join("bots.json")));
        let mut next_id = 0;
        let bot_ids = (0..bots.max_bots() + 100)
            .map(|index| {
                let data = (next_id, Bot { index });
                next_id += 1;
                data
            })
            .collect();
        Self {
            level,
            config,
            bots,
            next_id,
            clients: default(),
            round: Round {
                track: Track { from: 0, to: 1 },
                to_be_qualified: 1,
            },
            qualified_players: default(),
            players: default(),
            bot_ids,
            round_timer: Timer::new(),
        }
    }
    fn tick(&mut self) {
        if self.round_timer.elapsed().as_secs_f64() > self.config.cat_move_time as f64
            || self.players.is_empty()
        {
            self.time_up();
        }
        let mut bots = self.bots.get(
            self.round.track,
            self.round_timer.elapsed().as_secs_f64() as f32,
        );
        let mut bot_updates = Vec::new();
        for &id in &self.players {
            if let Some(bot) = self.bot_ids.get(&id) {
                if let Some(player) = bots.next() {
                    bot_updates.push((id, player));
                }
            }
        }
        mem::drop(bots);
        for (id, player) in bot_updates {
            self.update_player(id, player);
        }
    }
    fn new_session(&mut self) {
        let start = thread_rng().gen_range(0..self.level.cat_locations.len());
        self.players =
            itertools::chain![self.clients.keys().copied(), self.bot_ids.keys().copied()].collect();
        if self.players.iter().all(|id| self.bot_ids.contains_key(id)) {
            self.players.clear();
        }
        self.new_round_from(start);
    }
    fn new_round_from(&mut self, from: usize) {
        self.round = Round {
            track: self.level.random_track_from(from),
            to_be_qualified: (self.players.len() + 1) / 2,
        };
        for client in self.clients.values_mut() {
            client
                .sender
                .send(ServerMessage::NewRound(self.round.clone()));
        }

        let start_pos = self.level.cat_locations[from];
        for id in &self.players {
            if let Some(client) = self.clients.get_mut(id) {
                client.pos = Some(start_pos);
                client
                    .sender
                    .send(ServerMessage::YouHaveBeenRespawned(start_pos));
            }
        }

        self.qualified_players.clear();
        self.round_timer = Timer::new();
        self.update_numbers();
    }
    fn player_finished(&mut self, id: Id) {
        assert!(self.qualified_players.len() < self.round.to_be_qualified);
        self.qualified_players.insert(id);
        if self.qualified_players.len() == self.round.to_be_qualified {
            self.end_round();
        }
        self.update_numbers();
        if let Some(client) = self.clients.get_mut(&id) {
            client.pos = None;
            client.sender.send(ServerMessage::YouHaveBeenQualified);
        }
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
        let numbers = Numbers {
            players_left,
            spectators,
            bots,
            qualified,
        };
        for client in self.clients.values_mut() {
            client.sender.send(ServerMessage::Numbers(numbers.clone()));
        }
    }

    fn end_round(&mut self) {
        if self.config.server_recordings {
            for client in self.clients.values_mut() {
                let replay = mem::replace(&mut client.current_replay, bots::MoveData::new());
                self.bots.push(self.round.track, replay);
            }
        }

        for (&id, client) in &mut self.clients {
            if !self.qualified_players.contains(&id) {
                client.sender.send(ServerMessage::YouHaveBeenEliminated);
            }
        }

        self.players
            .retain(|id| self.qualified_players.contains(id));
        if self.players.len() <= 1 {
            // TODO: wait a bit to congratulate the winner
            self.new_session();
        } else {
            self.new_round_from(self.round.track.to);
        }
    }

    fn update_player(&mut self, id: Id, player: Player) {
        if let Some(client) = self.clients.get(&id) {
            if client.pos.is_none() {
                // Ignore, is you cheating???
                return;
            }
        }

        for (&client_id, client) in &mut self.clients {
            if client_id == id {
                client.pos = Some(player.pos);
                if !self.qualified_players.contains(&id) && self.config.server_recordings {
                    client.current_replay.push(
                        self.round_timer.elapsed().as_secs_f64() as f32,
                        player.clone(),
                    );
                }
            } else {
                client
                    .sender
                    .send(ServerMessage::UpdatePlayer(id, Some(player.clone())));
            }
        }

        self.check_finished(id, player);
    }

    fn check_finished(&mut self, id: Id, player: Player) {
        if self.qualified_players.contains(&id) {
            return;
        }
        if player.vel.len() > 1e5 {
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
        state.update_numbers();
    }
}

impl geng::net::Receiver<ClientMessage> for ClientConnection {
    fn handle(&mut self, message: ClientMessage) {
        let mut state = self.state.lock().unwrap();
        let state: &mut State = state.deref_mut();
        match message {
            ClientMessage::Ping => {
                state
                    .clients
                    .get_mut(&self.id)
                    .expect("Sender not found for client")
                    .sender
                    .send(ServerMessage::Pong);
            }
            ClientMessage::UpdatePlayer(player) => {
                state.update_player(self.id, player);
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
        sender: Box<dyn geng::net::Sender<Self::ServerMessage>>,
    ) -> ClientConnection {
        let mut state = self.state.lock().unwrap();
        let id = state.next_id;
        state.clients.insert(
            id,
            Client {
                current_replay: bots::MoveData::new(),
                pos: None,
                sender,
            },
        );
        state.next_id += 1;
        state.update_numbers();
        ClientConnection {
            id,
            state: self.state.clone(),
        }
    }
}
