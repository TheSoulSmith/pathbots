use std::cmp::Ordering::*;

/// Stores a 16x16 chunk of paths
struct Chunk {
    cont: [[Box<dyn Path>; 16]; 16],
    coord: ChunkCoord,
    id: usize,
}

pub enum BotState {
    Ok,
    Terminate,
}

/// Defines a struct as qualifying as a path
pub trait Path {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> BotState;
    fn flyover(&mut self, bot: &mut Bot, world: &mut World);
    fn store(&self, out: &mut Vec<u8>);
    fn load(from: LoadingChunk) -> Self
    where
        Self: Sized;
}

/// This exists because I am too lazy to figure out how to use std::io::Read
pub struct LoadingChunk {
    data: Vec<u8>,
    pos: usize,
}

/// Stores a single bot and all the information it needs to function
pub struct Bot {
    pos: BotPos,
    fly_height: usize,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
struct ChunkCoord {
    x: isize,
    y: isize,
}

struct BotPos {
    chunk: ChunkCoord,
    chunk_id: usize,
    in_chunk: u8,
    facing: u8,
}

impl BotPos {
    fn forward(&mut self, world: &World) {
        let (dir_x, dir_y) = match self.facing {
            0 => (1, 0),
            1 => (0, 1),
            2 => (-1, 0),
            3 => (0, -1),
            _ => panic!("This should be unreachable"),
        };
        self.shift(world, dir_x, dir_y);
    }
    fn shift(&mut self, world: &World, dir_x: isize, dir_y: isize) {
        let mut in_chunk_x = self.in_chunk as isize % 16 + dir_x;
        let mut in_chunk_y = self.in_chunk as isize / 16 + dir_y;
        if (in_chunk_x >= 16) | (in_chunk_x <= -16) {
            self.chunk.x += in_chunk_x.div_euclid(16);
            in_chunk_x = in_chunk_x.rem_euclid(16);
        };
        if (in_chunk_y >= 16) | (in_chunk_y <= -16) {
            self.chunk.x += in_chunk_y.div_euclid(16);
            in_chunk_y = in_chunk_y.rem_euclid(16);
        };
        self.in_chunk = in_chunk_y as u8 * 16 + in_chunk_x as u8;
        self.chunk_id = world.request_chunk_id(self.chunk);
    }
    fn update_chunk_id(&mut self, coord: ChunkCoord) {
        match self.chunk.cmp(&coord) {
            Greater => {}
            Equal => panic!("This should be unreachable"),
            Less => self.chunk_id += 1,
        }
    }
}

/// Stores a world, including chunks and bots
pub struct World {
    chunks: Vec<Chunk>,
    bots: Vec<Bot>,
}

impl World {
    fn request_chunk_id(&self, pos: ChunkCoord) -> usize {
        match self.chunks.binary_search_by_key(&pos, |chunk| chunk.coord) {
            Ok(t) => t,
            Err(_) => panic!(
                "Couldn't find the chunk and we do not yet have a way to load or generate it"
            ),
        }
    }
}
