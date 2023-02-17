use super::*;

struct State {
    next_id: Id,
    senders: HashMap<Id, Box<dyn geng::net::Sender<ServerMessage>>>,
}

impl State {
    fn new() -> Self {
        Self {
            next_id: 0,
            senders: default(),
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
                loop {
                    {
                        let cat_location = thread_rng().gen_range(0..level.cat_locations.len());
                        let mut state = state.lock().unwrap();
                        for sender in state.senders.values_mut() {
                            sender.send(ServerMessage::UpdateCat(Some(cat_location)));
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_secs(10));
                }
            }),
        }
    }
}

pub struct Client {
    id: Id,
    state: Arc<Mutex<State>>,
}

impl Drop for Client {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.senders.remove(&self.id);
        for other in state.senders.values_mut() {
            other.send(ServerMessage::Disconnect(self.id));
        }
    }
}

impl geng::net::Receiver<ClientMessage> for Client {
    fn handle(&mut self, message: ClientMessage) {
        let mut state = self.state.lock().unwrap();
        let sender = state
            .senders
            .get_mut(&self.id)
            .expect("Sender not found for client");
        match message {
            ClientMessage::Ping => sender.send(ServerMessage::Pong),
            ClientMessage::UpdatePlayer(player) => {
                for (id, sender) in &mut state.senders {
                    if *id != self.id {
                        sender.send(ServerMessage::UpdatePlayer(self.id, player.clone()));
                    }
                }
            }
        }
    }
}

impl geng::net::server::App for App {
    type Client = Client;
    type ServerMessage = ServerMessage;
    type ClientMessage = ClientMessage;
    fn connect(&mut self, sender: Box<dyn geng::net::Sender<Self::ServerMessage>>) -> Client {
        let mut state = self.state.lock().unwrap();
        let id = state.next_id;
        state.senders.insert(id, sender);
        state.next_id += 1;
        Client {
            id,
            state: self.state.clone(),
        }
    }
}
