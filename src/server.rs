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
                let mut cat_pos = vec2::ZERO;
                loop {
                    {
                        let mut state = state.lock().unwrap();
                        if state.clients.values().any(|client| client.pos.is_some()) {
                            for client in state.clients.values_mut() {
                                if let Some(client_pos) = client.pos {
                                    if (client_pos - cat_pos).len() > config.player_radius * 2.0 {
                                        client.sender.send(ServerMessage::YouHaveBeenEliminated);
                                        client.pos = None;
                                    }
                                }
                            }
                        } else {
                            for client in state.clients.values_mut() {
                                let pos = vec2::ZERO;
                                client.pos = Some(pos);
                                client.sender.send(ServerMessage::YouHaveBeenRespawned(pos));
                            }
                        }
                        let cat_pos_index = thread_rng().gen_range(0..level.cat_locations.len());
                        cat_pos = level.cat_locations[cat_pos_index];
                        for client in state.clients.values_mut() {
                            client
                                .sender
                                .send(ServerMessage::UpdateCat(Some(cat_pos_index)));
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_secs(10));
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
                for (id, client) in &mut state.clients {
                    if *id == self.id {
                        if client.pos.is_none() {
                            // error!("YO, someone is cheating!");
                        } else {
                            client.pos = Some(player.pos);
                        }
                    } else {
                        client
                            .sender
                            .send(ServerMessage::UpdatePlayer(self.id, player.clone()));
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
