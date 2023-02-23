use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub skin: usize,
    pub pos: vec2<f32>,
    pub vel: vec2<f32>,
    pub rot: f32,
}

#[derive(Serialize, Deserialize)]
struct TimedData {
    time: f32,
    data: PlayerSnapshot,
}

#[derive(Serialize, Deserialize)]
pub struct MoveData {
    data: Vec<TimedData>,
}

pub struct Result {
    pub time: f32,
    pub pos: vec2<f32>,
}

impl MoveData {
    pub fn new() -> Self {
        Self { data: vec![] }
    }
    pub fn push(&mut self, time: f32, data: Player) {
        self.data.push(TimedData {
            time,
            data: PlayerSnapshot {
                skin: 0,
                pos: data.pos,
                vel: data.vel,
                rot: data.rot,
            },
        });
    }
    pub fn get(&self, time: f32) -> Player {
        let index = match self
            .data
            .binary_search_by_key(&r32(time), |data| r32(data.time))
        {
            Ok(index) => index,
            Err(index) => index.max(1) - 1,
        };
        let p1 = &self.data[index];
        let p2 = &self.data[(index + 1).min(self.data.len() - 1)];
        let t = (time - p1.time) / (p2.time - p1.time);
        Player {
            skin: 0,
            color: 0.0,
            pos: p1.data.pos * (1.0 - t) + p2.data.pos * t,
            vel: p1.data.vel * (1.0 - t) + p2.data.vel * t,
            rot: p1.data.rot * (1.0 - t) + p2.data.rot * t,
        }
    }

    fn result(&self) -> Result {
        let data = self.data.last().unwrap();
        Result {
            time: data.time,
            pos: data.data.pos,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Data(HashMap<Track, Vec<MoveData>>);

impl Data {
    pub async fn load(path: impl AsRef<std::path::Path>) -> Self {
        async fn load(path: impl AsRef<std::path::Path>) -> anyhow::Result<Data> {
            let bytes = file::load_bytes(path).await?;
            Ok(bincode::deserialize(&bytes)?)
        }
        load(path).await.unwrap_or(Self(default()))
    }

    pub fn push(&mut self, track: Track, replay: MoveData) {
        if replay.data.is_empty() {
            return;
        }
        self.0.entry(track).or_default().push(replay);
    }

    pub fn get(&self, track: Track, time: f32) -> impl Iterator<Item = Player> + '_ {
        self.0
            .get(&track)
            .into_iter()
            .flat_map(move |data| data.iter().map(move |data| data.get(time)))
    }

    pub fn max_bots(&self) -> usize {
        self.0.values().map(|data| data.len()).min().unwrap_or(0)
    }

    pub fn get_results(&self, track: Track) -> impl Iterator<Item = Result> + '_ {
        self.0
            .get(&track)
            .into_iter()
            .flat_map(move |data| data.iter().map(move |data| data.result()))
    }
}
