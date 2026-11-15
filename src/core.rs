use crate::paths::{NoPath, PathKind};
use flate2::{
    Compression,
    write::{GzDecoder, GzEncoder},
};
use std::{
    cmp::Ordering::*,
    collections::{BTreeMap, VecDeque},
    fs::{create_dir_all, read, write},
    io::Write,
};

const REG_MAGIC: &[u8; 4] = b"HLID";
const CHUNK_MAGIC: &[u8; 4] = b"PBCH";
const CHUNK_FORMAT_VERSION: u16 = 1;
const CHUNK_SIDE: usize = 16;

/// Stores a 16x16 chunk of paths
pub(crate) struct Chunk {
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
pub(crate) enum BotState {
    Ok,
    Terminate,
}

/// Defines a struct as qualifying as a path
pub(crate) trait Path {
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

pub(crate) struct LoadingChunk {
    data: Vec<u8>,
    pos: usize,
}

impl LoadingChunk {
    fn new(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.get(..4) != Some(REG_MAGIC.as_slice()) {
            return Err(format!("File magic does not match"));
        }
        if bytes.get(4..6) != Some(CHUNK_FORMAT_VERSION.to_be_bytes().as_slice()) {
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
    pub(crate) fn read_u8(&mut self) -> Option<u8> {
        let out = self.data.get(self.pos).copied()?;
        self.pos += 1;
        Some(out)
    }
    pub(crate) fn read_i32(&mut self) -> Option<i32> {
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
pub(crate) struct Bot {
    pos: BotPos,
    fly_height: u8,
    vars: BTreeMap<String, Value>,
    action_time: u64,
    run_when: u64,
}

impl Bot {
    pub(crate) fn forward(&mut self, world: &mut World) -> Result<(), String> {
        self.pos.forward(world)
    }
    pub(crate) fn turn(&mut self, turn: i8) {
        self.pos.turn(turn);
    }
    pub(crate) fn set_dir(&mut self, dir: u8) {
        self.pos.facing = dir & 3
    }
    pub(crate) fn shift(
        &mut self,
        world: &mut World,
        dir_x: i32,
        dir_y: i32,
    ) -> Result<(), String> {
        self.pos.shift(world, dir_x, dir_y)
    }
    pub(crate) fn set_action_time(&mut self, action_time: u64) {
        self.action_time = std::cmp::max(1, action_time);
    }
    pub(crate) fn change_action_time(&mut self, change: i64) {
        self.action_time = std::cmp::max(1, self.action_time.saturating_add_signed(change));
    }
}

pub(crate) enum Value {
    Str(String),
    Num(f64),
    List(Vec<Value>),
    Dict(BTreeMap<String, Value>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub(crate) struct ChunkCoord {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

impl ChunkCoord {
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

pub(crate) struct BotPos {
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
    fn turn(&mut self, turn: i8) {
        self.facing = self.facing.wrapping_add_signed(turn) & 3
    }
    fn shift(&mut self, world: &mut World, dir_x: i32, dir_y: i32) -> Result<(), String> {
        let mut in_chunk_x = (self.in_chunk as i32 & 15) + dir_x;
        let mut in_chunk_y = self.in_chunk as i32 / 16 + dir_y;
        if !(0..16).contains(&in_chunk_x) {
            self.chunk.x += in_chunk_x.div_euclid(16);
            in_chunk_x = in_chunk_x.rem_euclid(16);
        };
        if !(0..16).contains(&in_chunk_y) {
            self.chunk.y += in_chunk_y.div_euclid(16);
            in_chunk_y = in_chunk_y.rem_euclid(16);
        };
        self.in_chunk = in_chunk_y as u8 * 16 + in_chunk_x as u8;
        self.chunk_id = world
            .request_chunk_id(self.chunk)
            .map_err(|e| format!("{}\nat bot pos shift", e))?;
        Ok(())
    }
    fn update_chunk_id(&mut self, index: usize, count: usize) {
        if self.chunk_id >= index {
            self.chunk_id += count
        }
    }
}

/// Stores a world, including chunks and bots
pub(crate) struct World {
    chunks: Vec<Chunk>,
    bots: VecDeque<Bot>,
    regions: Vec<(i32, i32)>,
    tick: u64,
    save_address: String,
}

impl World {
    pub(crate) fn new(save_address: String) -> Self {
        Self {
            chunks: Vec::new(),
            bots: VecDeque::new(),
            regions: Vec::new(),
            tick: 0,
            save_address,
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
                        self.load(format!("{}/reg_{}_{}.dat", self.save_address, reg_x, reg_y))
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
    pub(crate) fn load_region(&mut self, reg_x: i32, reg_y: i32) -> Result<(), String> {
        match self.regions.binary_search(&(reg_x, reg_y)) {
            Ok(_) => Ok(()),
            Err(index) => {
                self.regions.insert(index, (reg_x, reg_y));
                self.load(format!("{}/reg_{}_{}.dat", self.save_address, reg_x, reg_y))
            }
        }
    }
    pub(crate) fn save(&self) -> Result<(), String> {
        if !self.chunks.is_empty() {
            create_dir_all(&self.save_address).map_err(|e| {
                format!("{}\nat creating save directory `{}`", e, self.save_address)
            })?;
        }

        let mut start = 0;
        while start < self.chunks.len() {
            let first = &self.chunks[start];
            let reg_x = first.coord.x.div_euclid(CHUNK_SIDE as i32);
            let reg_y = first.coord.y.div_euclid(CHUNK_SIDE as i32);
            let mut end = start + 1;
            while end < self.chunks.len()
                && self.chunks[end].coord.x.div_euclid(CHUNK_SIDE as i32) == reg_x
                && self.chunks[end].coord.y.div_euclid(CHUNK_SIDE as i32) == reg_y
            {
                end += 1;
            }

            let mut data = vec![0; 4];
            for chunk in &self.chunks[start..end] {
                data.extend_from_slice(CHUNK_MAGIC);
                data.extend_from_slice(&chunk.coord.x.to_be_bytes());
                data.extend_from_slice(&chunk.coord.y.to_be_bytes());
                for column in chunk.cont.iter() {
                    for path in column.iter() {
                        data.push(path.kind() as u8);
                        path.store(&mut data);
                    }
                }
            }

            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(&data)
                .map_err(|e| format!("{}\nat compressing region {}, {}", e, reg_x, reg_y))?;
            let compressed = encoder
                .finish()
                .map_err(|e| format!("{}\nat finishing region {}, {}", e, reg_x, reg_y))?;

            let mut file_data = Vec::with_capacity(6 + compressed.len());
            file_data.extend_from_slice(REG_MAGIC);
            file_data.extend_from_slice(&CHUNK_FORMAT_VERSION.to_be_bytes());
            file_data.extend_from_slice(&compressed);
            let file_path = format!("{}/reg_{}_{}.dat", self.save_address, reg_x, reg_y);
            write(&file_path, file_data)
                .map_err(|e| format!("{}\nat saving region file `{}`", e, file_path))?;
            start = end;
        }

        Ok(())
    }
    fn update_bot_chunk_ids(&mut self, index: usize, count: usize) {
        for bot in &mut self.bots {
            bot.pos.update_chunk_id(index, count);
        }
    }
    pub(crate) fn tick_bots(&mut self) -> Result<(), String> {
        let result = (|| {
            let bot_count = self.bots.len();
            let mut processed_survivors = 0;
            for _ in 0..bot_count {
                let mut bot = self
                    .bots
                    .pop_front()
                    .expect("bot count is fixed for this tick");
                if bot.run_when != self.tick {
                    self.bots.push_back(bot);
                    processed_survivors += 1;
                    continue;
                }

                let path_chunk = bot.pos.chunk;
                let path_chunk_id = bot.pos.chunk_id;
                let path_in_chunk = bot.pos.in_chunk as usize;
                let chunk_count = self.chunks.len();
                let mut path = std::mem::replace(
                    &mut self.chunks[path_chunk_id].cont[path_in_chunk / 16][path_in_chunk & 15],
                    Box::new(NoPath()),
                );

                let outcome = if bot.fly_height == 0 {
                    path.bot(&mut bot, self)
                } else {
                    path.flyover(&mut bot, self)
                };
                bot.run_when += bot.action_time;

                let chunks_changed = self.chunks.len() != chunk_count;
                let path_chunk_id = if chunks_changed {
                    self.chunks
                        .binary_search_by_key(&path_chunk, |loaded| loaded.coord)
                        .expect("a bot's path chunk cannot be removed during its execution")
                } else {
                    path_chunk_id
                };
                self.chunks[path_chunk_id].cont[path_in_chunk / 16][path_in_chunk & 15] = path;

                if chunks_changed {
                    bot.pos.chunk_id = self
                        .chunks
                        .binary_search_by_key(&bot.pos.chunk, |loaded| loaded.coord)
                        .expect("a bot's current chunk cannot be removed during its execution");
                }

                match outcome.map_err(|e| format!("{}\nat bot execution", e)) {
                    Ok(BotState::Terminate) => {}
                    Ok(BotState::Ok) => {
                        self.bots.push_back(bot);
                        processed_survivors += 1;
                    }
                    Err(error) => {
                        self.bots.push_front(bot);
                        self.bots.rotate_right(processed_survivors);
                        return Err(error);
                    }
                }
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
    use crate::paths::BasicPath;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
        let mut world = World::new("test_save".to_string());
        world.chunks.push(Chunk::new_empty(coord));

        for (in_chunk, fly_height, action_time) in [(0, 0, 2), (1, 1, 3)] {
            world.bots.push_back(Bot {
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

    #[test]
    fn tick_bots_removes_terminated_bot_and_continues_with_next_bot() {
        let coord = ChunkCoord { x: 0, y: 0 };
        let mut world = World::new("test_save".to_string());
        world.chunks.push(Chunk::new_empty(coord));
        world.bots.push_back(Bot {
            pos: BotPos {
                chunk: coord,
                chunk_id: 0,
                in_chunk: 0,
                facing: 0,
            },
            fly_height: 0,
            vars: BTreeMap::new(),
            action_time: 1,
            run_when: 0,
        });
        world.bots.push_back(Bot {
            pos: BotPos {
                chunk: coord,
                chunk_id: 0,
                in_chunk: 1,
                facing: 0,
            },
            fly_height: 0,
            vars: BTreeMap::new(),
            action_time: 1,
            run_when: 0,
        });
        world.chunks[0].cont[0][1] = Box::new(RecordingPath);

        world.tick_bots().unwrap();

        assert_eq!(world.bots.len(), 1);
        assert!(matches!(
            world.bots[0].vars.get("execution"),
            Some(Value::Str(execution)) if execution == "bot"
        ));
        assert_eq!(world.bots[0].run_when, 1);
    }

    #[test]
    fn tick_bots_keeps_cached_chunk_ids_correct_when_a_chunk_is_inserted() {
        let coord = ChunkCoord { x: 0, y: 0 };
        let mut world = World::new("test_save".to_string());
        world.chunks.push(Chunk::new_empty(coord));
        world.bots.push_back(Bot {
            pos: BotPos {
                chunk: coord,
                chunk_id: 0,
                in_chunk: 0,
                facing: 2,
            },
            fly_height: 0,
            vars: BTreeMap::new(),
            action_time: 1,
            run_when: 0,
        });
        world.bots.push_back(Bot {
            pos: BotPos {
                chunk: coord,
                chunk_id: 0,
                in_chunk: 1,
                facing: 0,
            },
            fly_height: 0,
            vars: BTreeMap::new(),
            action_time: 1,
            run_when: 0,
        });
        world.chunks[0].cont[0][0] = Box::new(BasicPath());
        world.chunks[0].cont[0][1] = Box::new(RecordingPath);

        world.tick_bots().unwrap();

        assert_eq!(world.bots[0].pos.chunk, ChunkCoord { x: -1, y: 0 });
        assert_eq!(world.bots[0].pos.chunk_id, 0);
        assert_eq!(world.bots[1].pos.chunk_id, 1);
        assert!(matches!(
            world.bots[1].vars.get("execution"),
            Some(Value::Str(execution)) if execution == "bot"
        ));
    }

    #[test]
    fn bot_forward_updates_the_y_chunk_coordinate() {
        let coord = ChunkCoord { x: 0, y: 0 };
        let mut world = World::new("test_save".to_string());
        world.chunks.push(Chunk::new_empty(coord));
        let mut pos = BotPos {
            chunk: coord,
            chunk_id: 0,
            in_chunk: 0,
            facing: 3,
        };

        pos.forward(&mut world).unwrap();

        assert_eq!(pos.chunk, ChunkCoord { x: 0, y: -1 });
        assert_eq!(pos.in_chunk, 240);
        assert_eq!(pos.chunk_id, 0);
    }

    #[test]
    fn save_round_trips_chunks_by_region() {
        static NEXT_SAVE_ID: AtomicUsize = AtomicUsize::new(0);
        let save_address = std::env::temp_dir().join(format!(
            "pathbots-save-{}-{}",
            std::process::id(),
            NEXT_SAVE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let save_address = save_address.to_string_lossy().into_owned();
        let mut world = World::new(save_address.clone());

        let mut negative_chunk = Chunk::new_empty(ChunkCoord { x: -1, y: -1 });
        negative_chunk.cont[0][0] = Box::new(BasicPath());
        world.chunks.push(negative_chunk);

        let mut positive_chunk = Chunk::new_empty(ChunkCoord { x: 16, y: 0 });
        positive_chunk.cont[15][15] = Box::new(BasicPath());
        world.chunks.push(positive_chunk);

        world.save().unwrap();

        let mut loaded = World::new(save_address.clone());
        loaded.load_region(-1, -1).unwrap();
        loaded.load_region(1, 0).unwrap();

        assert_eq!(loaded.chunks.len(), 2);
        assert_eq!(loaded.chunks[0].coord, ChunkCoord { x: -1, y: -1 });
        assert_eq!(loaded.chunks[0].cont[0][0].kind(), PathKind::BasicPath);
        assert_eq!(loaded.chunks[1].coord, ChunkCoord { x: 16, y: 0 });
        assert_eq!(loaded.chunks[1].cont[15][15].kind(), PathKind::BasicPath);

        std::fs::remove_dir_all(save_address).unwrap();
    }
}
