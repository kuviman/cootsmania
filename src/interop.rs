use super::*;

pub type Id = i64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub color: f32,
    pub skin: usize,
    pub pos: vec2<f32>,
    pub vel: vec2<f32>,
    pub rot: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ClientMessage {
    Ping,
    UpdatePlayer(Player),
    Name(String),
    Ready,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Numbers {
    pub players_left: usize,
    pub spectators: usize,
    pub bots: usize,
    pub qualified: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Round {
    pub num: usize,
    pub track: Track,
    pub to_be_qualified: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerMessage {
    Pong,
    UpdatePlayer(Id, Option<Player>),
    Disconnect(Id),
    YouHaveBeenEliminated,
    YouHaveBeenRespawned(vec2<f32>),
    Numbers(Numbers), // TODO
    NewRound(Round),
    YouHaveBeenQualified,
    Name(Id, String),
    YouAreWinner,
    Winner(Option<Id>),
    YourName(String),
}
