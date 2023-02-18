use super::*;

struct Client {
    pos: Option<vec2<f32>>,
    sender: Box<dyn geng::net::Sender<ServerMessage>>,
}

struct State {
    next_id: Id,
    clients: HashMap<Id, Client>,
}

impl State {
    fn new() -> Self {
        Self {
            next_id: 0,
            clients: default(),
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
                let level: Level = serde_json::from_reader(
                    std::fs::File::open(run_dir().join("level.json")).unwrap(),
                )
                .unwrap();
                let config: Config = serde_json::from_reader(
                    std::fs::File::open(run_dir().join("config.json")).unwrap(),
                )
                .unwrap();
                let mut cat_pos_index = 0;
                let mut cat_pos = vec2::ZERO;
                let mut cat_move_time = config.cat_move_time;
                loop {
                    {
                        let mut state = state.lock().unwrap();
                        if state.clients.values().any(|client| client.pos.is_some()) {
                            let mut eliminated = Vec::new();
                            for (&id, client) in &state.clients {
                                if let Some(client_pos) = client.pos {
                                    if (client_pos - cat_pos).len() > config.player_radius * 2.0 {
                                        eliminated.push(id);
                                    }
                                }
                            }
                            if eliminated.is_empty() {
                                cat_move_time -= config.cat_move_time_change;
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

                            // Everyone eliminated?
                            if !state.clients.values().any(|client| client.pos.is_some()) {
                                cat_move_time = config.new_round_time;
                            }
                        } else {
                            for client in state.clients.values_mut() {
                                let pos = vec2::ZERO;
                                client.pos = Some(pos);
                                client.sender.send(ServerMessage::YouHaveBeenRespawned(pos));
                            }
                            cat_move_time = config.cat_move_time;
                        }
                        cat_pos_index = loop {
                            let index = thread_rng().gen_range(0..level.cat_locations.len());
                            if index != cat_pos_index {
                                break index;
                            }
                        };
                        cat_pos = level.cat_locations[cat_pos_index];
                        for client in state.clients.values_mut() {
                            client.sender.send(ServerMessage::UpdateCat {
                                location: Some(cat_pos_index),
                                move_time: cat_move_time,
                            });
                        }
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
        state.clients.insert(id, Client { pos: None, sender });
        state.next_id += 1;
        ClientConnection {
            id,
            state: self.state.clone(),
        }
    }
}
