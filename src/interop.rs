use super::*;

pub type Id = i64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub skin: usize,
    pub pos: vec2<f32>,
    pub vel: vec2<f32>,
    pub rot: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ClientMessage {
    Ping,
    UpdatePlayer(Player),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerMessage {
    Pong,
    UpdatePlayer(Id, Option<Player>),
    Disconnect(Id),
    UpdateCat {
        bots: usize,
        location: Option<usize>,
        move_time: f32,
    },
    YouHaveBeenEliminated,
    YouHaveBeenRespawned(vec2<f32>),
    YouScored(i32),
    UpdatePlacement(usize),
}
