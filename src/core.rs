use crate::paths::{BasicPath, NoPath};
use flate2::write::GzDecoder;
use std::{cmp::Ordering::*, collections::BTreeMap, fs::read, io::Write};

const REG_MAGIC: &[u8; 4] = b"HLID";
const CHUNK_MAGIC: &[u8; 4] = b"PBCH";
const CHUNK_FORMAT_VERSION: u16 = 1;
const CHUNK_SIDE: usize = 16;

/// Stores a 16x16 chunk of paths
pub struct Chunk {
    cont: Box<[[Box<dyn Path>; CHUNK_SIDE]; CHUNK_SIDE]>,
    coord: ChunkCoord,
}

impl Chunk {
    fn new_empty(coord: ChunkCoord) -> Self {
        Self {
            cont: Box::new(std::array::from_fn(|_| {
                std::array::from_fn(|_| Box::new(NoPath()) as Box<dyn Path>)
            })),
            coord,
        }
    }
    fn load(from: &mut LoadingChunk) -> Result<Self, String> {
        if from.read_4() != Some(*CHUNK_MAGIC) {
            return Err(format!("Magic does not match"));
        }
        let coord = match ChunkCoord::load(from) {
            Ok(t) => t,
            Err(e) => return Err(format!("{}\nat coord", e)),
        };
        let mut cont_vec: Vec<[Box<dyn Path + 'static>; 16]> = Vec::new();
        for x in 0u8..16 {
            let mut col_vec: Vec<Box<dyn Path>> = Vec::new();
            for y in 0u8..16 {
                col_vec.push(
                    match PathKind::from_id(match from.read_u8() {
                        Some(t) => t,
                        None => {
                            return Err(format!(
                                "Could not read path id at {}, {}\nat chunk {}, {}",
                                x, y, coord.x, coord.y
                            ));
                        }
                    }) {
                        Ok(t) => match t.load(from) {
                            Ok(o) => o,
                            Err(e) => {
                                return Err(format!(
                                    "{}\nat path {}, {}\nat chunk {}, {}",
                                    e, x, y, coord.x, coord.y
                                ));
                            }
                        },
                        Err(e) => {
                            return Err(format!(
                                "{}\nat path {}, {}\nat chunk {}, {}",
                                e, x, y, coord.x, coord.y
                            ));
                        }
                    },
                );
            }
            cont_vec.push(match col_vec.try_into() {
                Ok(t) => t,
                Err(_) => {
                    return Err(format!(
                        "Could not reshape column {}\nat chunk {}, {}",
                        x, coord.x, coord.y
                    ));
                }
            });
        }
        Ok(Self {
            cont: match cont_vec.try_into() {
                Ok(t) => Box::new(t),
                Err(_) => {
                    return Err(format!(
                        "Could not reshape chunk\nat chunk {}, {}",
                        coord.x, coord.y
                    ));
                }
            },
            coord,
        })
    }
}

#[derive(Clone, Copy)]
pub enum BotState {
    Ok,
    Terminate,
}

/// Defines a struct as qualifying as a path
pub trait Path {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String>;
    fn flyover(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, out: &mut Vec<u8>);
    fn kind(&self) -> PathKind;
    fn load(from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized;
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Stable on-disk identifiers for path types; never reuse an assigned value.
pub enum PathKind {
    NoPath = 0,
    BasicPath = 1,
}

impl PathKind {
    fn from_id(id: u8) -> Result<Self, String> {
        match id {
            0 => Ok(Self::NoPath),
            1 => Ok(Self::BasicPath),
            e => Err(format!("Path id {} is not mapped to a path", e)),
        }
    }

    fn load(self, from: &mut LoadingChunk) -> Result<Box<dyn Path>, String> {
        match self {
            Self::NoPath => Ok(Box::new(NoPath::load(from)?)),
            Self::BasicPath => Ok(Box::new(BasicPath::load(from)?)),
        }
    }
}

pub struct LoadingChunk {
    data: Vec<u8>,
    pos: usize,
}

impl LoadingChunk {
    fn new(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes[0..4] != *REG_MAGIC {
            return Err(format!("File magic does not match"));
        }
        if bytes[2..6] != CHUNK_FORMAT_VERSION.to_be_bytes() {
            return Err(format!("File format does not match"));
        }
        let mut data = Vec::new();
        let mut decoder = GzDecoder::new(data);
        match decoder.write_all(&bytes[6..]) {
            Ok(_) => {}
            Err(e) => return Err(format!("{}\nat gzip decoding", e)),
        };
        data = match decoder.finish() {
            Ok(t) => t,
            Err(e) => return Err(format!("{}\nat gzip decoder finishing", e)),
        };
        Ok(Self { data, pos: 4 })
    }
    fn read_u8(&mut self) -> Option<u8> {
        let out = self.data.get(self.pos).copied()?;
        self.pos += 1;
        Some(out)
    }
    fn read_i32(&mut self) -> Option<i32> {
        let end = self.pos.checked_add(4)?;
        let bytes = self.data.get(self.pos..end)?;
        let array: [u8; 4] = bytes.try_into().ok()?;
        self.pos = end;
        Some(i32::from_be_bytes(array))
    }
    fn read_4(&mut self) -> Option<[u8; 4]> {
        let end = self.pos.checked_add(4)?;
        let bytes = self.data.get(self.pos..end)?;
        let array: [u8; 4] = bytes.try_into().ok()?;
        self.pos = end;
        Some(array)
    }
    fn is_done(&self) -> bool {
        self.data.len() == self.pos
    }
}

impl Iterator for LoadingChunk {
    type Item = Result<Chunk, String>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.is_done() {
            None
        } else {
            Some(Chunk::load(self))
        }
    }
}

/// Stores a single bot and all the information it needs to function
pub struct Bot {
    pos: BotPos,
    fly_height: usize,
    vars: BTreeMap<String, Value>,
    action_time: usize,
    run_when: usize,
}

impl Bot {
    pub fn forward(&mut self, world: &mut World) -> Result<(), String> {
        self.pos.forward(world)
    }
}

pub enum Value {
    Str(String),
    Num(f64),
    List(Vec<Value>),
    Dict(BTreeMap<String, Value>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub struct ChunkCoord {
    pub x: i32,
    pub y: i32,
}

impl ChunkCoord {
    fn reg_name(&self) -> String {
        format!(
            "reg_{}_{}.dat",
            self.x.div_euclid(16),
            self.y.div_euclid(16)
        )
    }
    fn load(from: &mut LoadingChunk) -> Result<Self, String> {
        match from.read_i32() {
            Some(x) => match from.read_i32() {
                Some(y) => Ok(Self { x, y }),
                None => Err(format!("Failed to read y value")),
            },
            None => Err(format!("Failed to read x value")),
        }
    }
}

impl Ord for ChunkCoord {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.y.div_euclid(16).cmp(&other.y.div_euclid(16)) {
            Equal => match self.x.div_euclid(16).cmp(&other.x.div_euclid(16)) {
                Equal => match self.y.cmp(&other.y) {
                    Equal => self.x.cmp(&other.x),
                    o => o,
                },
                o => o,
            },
            o => o,
        }
    }
}

struct BotPos {
    /// The coordinates of the chunk this bot is in
    chunk: ChunkCoord,
    /// The slot in the world that the chunk this bot is in currently occupies
    chunk_id: usize,
    in_chunk: u8,
    facing: u8,
}

impl BotPos {
    fn forward(&mut self, world: &mut World) -> Result<(), String> {
        let (dir_x, dir_y) = match self.facing {
            0 => (1, 0),
            1 => (0, 1),
            2 => (-1, 0),
            3 => (0, -1),
            _ => panic!("This should be unreachable"),
        };
        self.shift(world, dir_x, dir_y)
            .map_err(|e| format!("{}\nat bot pos forward shift", e))
    }
    fn shift(&mut self, world: &mut World, dir_x: i32, dir_y: i32) -> Result<(), String> {
        let mut in_chunk_x = self.in_chunk as i32 & 15 + dir_x;
        let mut in_chunk_y = self.in_chunk as i32 / 16 + dir_y;
        if (in_chunk_x >= 16) | (in_chunk_x <= -16) {
            self.chunk.x += in_chunk_x.div_euclid(16);
            in_chunk_x = in_chunk_x.rem_euclid(16);
        };
        if (in_chunk_y >= 16) | (in_chunk_y <= -16) {
            self.chunk.x += in_chunk_y.div_euclid(16);
            in_chunk_y = in_chunk_y.rem_euclid(16);
        };
        self.in_chunk = in_chunk_y as u8 * 16 + in_chunk_x as u8;
        self.chunk_id = world
            .request_chunk_id(self.chunk)
            .map_err(|e| format!("{}\nat bot pos shift", e))?;
        Ok(())
    }
    fn update_chunk_id(&mut self, index: usize, count: usize) {
        if self.chunk_id > index {
            self.chunk_id += count
        }
    }
}

/// Stores a world, including chunks and bots
pub struct World {
    chunks: Vec<Chunk>,
    bots: Vec<Bot>,
    regions: Vec<(i32, i32)>,
    tick: usize,
}

impl World {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            bots: Vec::new(),
            regions: Vec::new(),
            tick: 0,
        }
    }

    fn request_chunk_id(&mut self, pos: ChunkCoord) -> Result<usize, String> {
        match self.chunks.binary_search_by_key(&pos, |chunk| chunk.coord) {
            Ok(t) => Ok(t),
            Err(index) => {
                let reg_x = pos.x.div_euclid(16);
                let reg_y = pos.y.div_euclid(16);
                match self.regions.binary_search(&(reg_x, reg_y)) {
                    Ok(_) => {
                        self.chunks.insert(index, Chunk::new_empty(pos));
                        self.update_bot_chunk_ids(index, 1);
                        Ok(index)
                    }
                    Err(reg_index) => {
                        self.regions.insert(reg_index, (reg_x, reg_y));
                        self.load(format!("reg_{}_{}.dat", reg_x, reg_y))
                            .map_err(|e| format!("{}\nat request chunk slot", e))?;
                        match self.chunks.binary_search_by_key(&pos, |chunk| chunk.coord) {
                            Ok(t) => Ok(t),
                            Err(e_index) => {
                                self.chunks.insert(e_index, Chunk::new_empty(pos));
                                self.update_bot_chunk_ids(e_index, 1);
                                Ok(e_index)
                            }
                        }
                    }
                }
            }
        }
    }
    fn load(&mut self, from: String) -> Result<(), String> {
        let data = match LoadingChunk::new(match read(from.clone()) {
            Ok(t) => t,
            Err(_) => return Ok(()),
        }) {
            Ok(t) => t,
            Err(e) => return Err(format!("{}\nat loading region file `{}`", e, from)),
        };

        let mut insert_chunks = Vec::new();

        for chunk in data {
            match chunk {
                Ok(t) => {
                    insert_chunks.push(t);
                }
                Err(e) => return Err(format!("{}\nat loading region file {}", e, from)),
            }
        }
        match self
            .chunks
            .binary_search_by_key(&insert_chunks.get(0).unwrap().coord, |chunk| chunk.coord)
        {
            Ok(_) => panic!("This should be unreachable"),
            Err(index) => {
                self.update_bot_chunk_ids(index, insert_chunks.len());
                self.chunks.splice(index..index, insert_chunks)
            }
        };

        Ok(())
    }
    pub fn load_region(&mut self, reg_x: i32, reg_y: i32) -> Result<(), String> {
        match self.regions.binary_search(&(reg_x, reg_y)) {
            Ok(_) => Ok(()),
            Err(index) => {
                self.regions.insert(index, (reg_x, reg_y));
                self.load(format!("reg_{}_{}.dat", reg_x, reg_y))
            }
        }
    }
    fn update_bot_chunk_ids(&mut self, index: usize, count: usize) {
        for bot in &mut self.bots {
            bot.pos.update_chunk_id(index, count);
        }
    }
    pub fn tick_bots(&mut self) -> Result<(), String> {
        let result = (|| {
            for index in 0..self.bots.len() {
                if self.bots[index].run_when != self.tick {
                    continue;
                }

                let path_chunk = self.bots[index].pos.chunk;
                let path_in_chunk = self.bots[index].pos.in_chunk as usize;
                let chunk_id = self
                    .request_chunk_id(path_chunk)
                    .map_err(|e| format!("{}\nat path check\nat bot execution", e))?;
                let mut path = std::mem::replace(
                    &mut self.chunks[chunk_id].cont[path_in_chunk / 16][path_in_chunk & 15],
                    Box::new(NoPath()),
                );
                let mut bot = self.bots.remove(index);

                let outcome = if bot.fly_height == 0 {
                    path.bot(&mut bot, self)
                } else {
                    path.flyover(&mut bot, self)
                };
                bot.run_when += bot.action_time;

                let chunk_id = self
                    .chunks
                    .binary_search_by_key(&path_chunk, |loaded| loaded.coord)
                    .expect("a bot's path chunk cannot be removed during its execution");
                self.chunks[chunk_id].cont[path_in_chunk / 16][path_in_chunk & 15] = path;
                bot.pos.chunk_id = self
                    .chunks
                    .binary_search_by_key(&bot.pos.chunk, |loaded| loaded.coord)
                    .expect("a bot's current chunk cannot be removed during its execution");
                let outcome = outcome.map_err(|e| format!("{}\nat bot execution", e));
                self.bots.insert(index, bot);

                outcome?;
            }
            Ok(())
        })();
        self.tick += 1;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RecordingPath;

    fn record_execution(bot: &mut Bot, execution: &str) {
        let count = match bot.vars.get("execution_count") {
            Some(Value::Num(count)) => count + 1.0,
            _ => 1.0,
        };
        bot.vars
            .insert("execution".to_string(), Value::Str(execution.to_string()));
        bot.vars
            .insert("execution_count".to_string(), Value::Num(count));
    }

    impl Path for RecordingPath {
        fn bot(&mut self, bot: &mut Bot, _world: &mut World) -> Result<BotState, String> {
            record_execution(bot, "bot");
            Ok(BotState::Ok)
        }

        fn flyover(&mut self, bot: &mut Bot, _world: &mut World) -> Result<BotState, String> {
            record_execution(bot, "flyover");
            Ok(BotState::Ok)
        }

        fn store(&self, _out: &mut Vec<u8>) {}

        fn kind(&self) -> PathKind {
            PathKind::BasicPath
        }

        fn load(_from: &mut LoadingChunk) -> Result<Self, String> {
            Ok(Self)
        }
    }

    #[test]
    fn tick_bots_dispatches_based_on_fly_height() {
        let coord = ChunkCoord { x: 0, y: 0 };
        let mut world = World::new();
        world.chunks.push(Chunk::new_empty(coord));

        for (in_chunk, fly_height, action_time) in [(0, 0, 2), (1, 1, 3)] {
            world.bots.push(Bot {
                pos: BotPos {
                    chunk: coord,
                    chunk_id: 0,
                    in_chunk,
                    facing: 0,
                },
                fly_height,
                vars: BTreeMap::new(),
                action_time,
                run_when: 0,
            });
            world.chunks[0].cont[0][in_chunk as usize] = Box::new(RecordingPath);
        }

        world.tick_bots().unwrap();

        assert!(matches!(
            world.bots[0].vars.get("execution"),
            Some(Value::Str(execution)) if execution == "bot"
        ));
        assert!(matches!(
            world.bots[1].vars.get("execution"),
            Some(Value::Str(execution)) if execution == "flyover"
        ));
        assert_eq!(world.tick, 1);
        assert_eq!(world.bots[0].run_when, 2);
        assert_eq!(world.bots[1].run_when, 3);

        world.tick_bots().unwrap();

        assert_eq!(world.tick, 2);
        assert!(matches!(
            world.bots[0].vars.get("execution"),
            Some(Value::Str(execution)) if execution == "bot"
        ));
        assert!(matches!(
            world.bots[1].vars.get("execution"),
            Some(Value::Str(execution)) if execution == "flyover"
        ));
        assert!(matches!(
            world.bots[0].vars.get("execution_count"),
            Some(Value::Num(count)) if *count == 1.0
        ));
        assert!(matches!(
            world.bots[1].vars.get("execution_count"),
            Some(Value::Num(count)) if *count == 1.0
        ));

        world.tick_bots().unwrap();

        assert_eq!(world.tick, 3);
        assert_eq!(world.bots[0].run_when, 4);
        assert_eq!(world.bots[1].run_when, 3);
        assert!(matches!(
            world.bots[0].vars.get("execution_count"),
            Some(Value::Num(count)) if *count == 2.0
        ));
        assert!(matches!(
            world.bots[1].vars.get("execution_count"),
            Some(Value::Num(count)) if *count == 1.0
        ));
    }
}
