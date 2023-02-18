use super::*;

struct Client {
    pos: Option<vec2<f32>>,
    score: i32,
    this_score: Option<i32>, // Score of current cat move
    sender: Box<dyn geng::net::Sender<ServerMessage>>,
}

struct State {
    next_id: Id,
    level: Level,
    config: Config,
    cat_pos: Option<usize>,
    clients: HashMap<Id, Client>,
    this_start: Timer,
    cat_move_time: f32,
}

impl State {
    fn new() -> Self {
        let level: Level =
            serde_json::from_reader(std::fs::File::open(run_dir().join("level.json")).unwrap())
                .unwrap();
        let config: Config =
            serde_json::from_reader(std::fs::File::open(run_dir().join("config.json")).unwrap())
                .unwrap();
        Self {
            level,
            config,
            next_id: 0,
            cat_pos: None,
            clients: default(),
            this_start: Timer::new(),
            cat_move_time: 0.0,
        }
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
            background_thread: std::thread::spawn(move || {
                let mut cat_pos_index = 0;
                let mut cat_pos = vec2::ZERO;
                let mut cat_move_time = state.lock().unwrap().config.cat_move_time;
                loop {
                    {
                        let mut state = state.lock().unwrap();
                        if state.clients.values().any(|client| client.pos.is_some()) {
                            let mut update_placements: Vec<Id> = Vec::new();
                            let mut eliminated = Vec::new();
                            for (&id, client) in &mut state.clients {
                                if let Some(score) = client.this_score.take() {
                                    client.score += score;
                                    update_placements.push(id);
                                } else if client.pos.is_some() {
                                    eliminated.push(id);
                                    update_placements.push(id);
                                }
                            }
                            if eliminated.is_empty() {
                                cat_move_time -= state.config.cat_move_time_change;
                            }
                            for id in eliminated {
                                for (&client_id, client) in &mut state.clients {
                                    if client_id == id {
                                        client.pos = None;
                                        client.sender.send(ServerMessage::YouHaveBeenEliminated);
                                    } else {
                                        client.sender.send(ServerMessage::UpdatePlayer(id, None));
                                    }
                                }
                            }

                            update_placements.sort_by_key(|id| {
                                let client = state.clients.get(id).unwrap();
                                let eliminated = client.pos.is_none();
                                (eliminated, -client.score)
                            });
                            for (rank, id) in update_placements.into_iter().enumerate() {
                                state
                                    .clients
                                    .get_mut(&id)
                                    .unwrap()
                                    .sender
                                    .send(ServerMessage::UpdatePlacement(rank + 1));
                            }

                            // Everyone eliminated?
                            if !state.clients.values().any(|client| client.pos.is_some()) {
                                cat_move_time = state.config.new_round_time;
                            }
                        } else {
                            // TODO: sometimes we go here if all alive players have just disconnected and not eliminated
                            for client in state.clients.values_mut() {
                                let pos = vec2::ZERO;
                                client.score = 0;
                                client.this_score = None;
                                client.pos = Some(pos);
                                client.sender.send(ServerMessage::YouHaveBeenRespawned(pos));
                            }
                            cat_move_time = state.config.cat_move_time;
                        }
                        cat_pos_index = loop {
                            let index = thread_rng().gen_range(0..state.level.cat_locations.len());
                            if index != cat_pos_index {
                                break index;
                            }
                        };
                        cat_pos = state.level.cat_locations[cat_pos_index];
                        for client in state.clients.values_mut() {
                            client.sender.send(ServerMessage::UpdateCat {
                                location: Some(cat_pos_index),
                                move_time: cat_move_time,
                            });
                        }
                        state.cat_pos = Some(cat_pos_index);
                        state.cat_move_time = cat_move_time;
                        state.this_start = Timer::new();
                    }
                    std::thread::sleep(std::time::Duration::from_secs_f64(
                        cat_move_time.max(0.0) as f64
                    ));
                }
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
        for other in state.clients.values_mut() {
            other.sender.send(ServerMessage::Disconnect(self.id));
        }
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
                if state.clients[&self.id].pos.is_none() {
                    // Ignore, is you cheating???
                } else {
                    for (id, client) in &mut state.clients {
                        if *id == self.id {
                            client.pos = Some(player.pos);
                            if client.this_score.is_none() && player.vel.len() < 1e-5 {
                                if let Some(index) = state.cat_pos {
                                    if let Some(&pos) = state.level.cat_locations.get(index) {
                                        if (player.pos - pos).len()
                                            < state.config.player_radius * 2.0
                                        {
                                            let score = ((state.cat_move_time
                                                - state.this_start.elapsed().as_secs_f64() as f32)
                                                .max(0.0)
                                                * 1000.0)
                                                as i32;
                                            client.sender.send(ServerMessage::YouScored(score));
                                            client.this_score = Some(score);
                                        }
                                    }
                                }
                            }
                        } else {
                            client
                                .sender
                                .send(ServerMessage::UpdatePlayer(self.id, Some(player.clone())));
                        }
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
        sender: Box<dyn geng::net::Sender<Self::ServerMessage>>,
    ) -> ClientConnection {
        let mut state = self.state.lock().unwrap();
        let id = state.next_id;
        state.clients.insert(
            id,
            Client {
                this_score: None,
                score: 0,
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
