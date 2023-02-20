use super::*;

#[derive(Serialize, Deserialize)]
struct TimedData {
    time: f32,
    data: Player,
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
        self.data.push(TimedData { time, data });
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
pub struct Data(HashMap<usize, HashMap<usize, Vec<MoveData>>>);

impl Data {
    pub async fn load(path: impl AsRef<std::path::Path>) -> Self {
        file::load_json(path)
            .await
            .expect("Failed to load bots data")
    }

    pub fn push(&mut self, prev: usize, next: usize, replay: MoveData) {
        self.0
            .entry(prev)
            .or_default()
            .entry(next)
            .or_default()
            .push(replay);
    }

    pub fn get(&self, prev: usize, next: usize, time: f32) -> impl Iterator<Item = Player> + '_ {
        self.0
            .get(&prev)
            .and_then(move |data| data.get(&next))
            .into_iter()
            .flat_map(move |data| data.iter().map(move |data| data.get(time)))
    }

    pub fn max_bots(&self) -> usize {
        self.0
            .values()
            .flat_map(|data| data.values())
            .map(|data| data.len())
            .min()
            .unwrap_or(0)
    }

    pub fn get_results(&self, prev: usize, next: usize) -> impl Iterator<Item = Result> + '_ {
        self.0
            .get(&prev)
            .and_then(move |data| data.get(&next))
            .into_iter()
            .flat_map(move |data| data.iter().map(move |data| data.result()))
    }
}
