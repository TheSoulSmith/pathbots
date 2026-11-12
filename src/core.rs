use crate::paths::{BasicPath, NoPath};
use std::cmp::Ordering::*;

const FILE_MAGIC: &[u8; 4] = b"HLID";
const CHUNK_MAGIC: &[u8; 4] = b"PBCH";
const CHUNK_FORMAT_VERSION: u16 = 1;
const CHUNK_SIDE: usize = 16;

/// Stores a 16x16 chunk of paths
pub struct Chunk {
    cont: Box<[[Box<dyn Path>; CHUNK_SIDE]; CHUNK_SIDE]>,
    coord: ChunkCoord,
}

impl Chunk {
    pub fn new_empty(coord: ChunkCoord) -> Self {
        Self {
            cont: Box::new(std::array::from_fn(|_| {
                std::array::from_fn(|_| Box::new(NoPath()) as Box<dyn Path>)
            })),
            coord,
        }
    }

    pub fn coord(&self) -> ChunkCoord {
        self.coord
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, ChunkError> {
        let mut out = Vec::new();
        out.extend_from_slice(CHUNK_MAGIC);
        out.extend_from_slice(&CHUNK_FORMAT_VERSION.to_le_bytes());
        out.extend_from_slice(
            &i64::try_from(self.coord.x)
                .map_err(|_| ChunkError::CoordinateOutOfRange)?
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &i64::try_from(self.coord.y)
                .map_err(|_| ChunkError::CoordinateOutOfRange)?
                .to_le_bytes(),
        );

        for row in self.cont.iter() {
            for path in row {
                let mut payload = Vec::new();
                path.store(&mut payload);
                let payload_len =
                    u32::try_from(payload.len()).map_err(|_| ChunkError::PayloadTooLarge)?;

                out.push(path.kind() as u8);
                out.extend_from_slice(&payload_len.to_le_bytes());
                out.extend_from_slice(&payload);
            }
        }

        Ok(out)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, ChunkError> {
        let mut input = LoadingChunk::new(data.to_vec(), ChunkCoord { x: 0, y: 0 });
        if input.read_bytes(CHUNK_MAGIC.len())? != CHUNK_MAGIC {
            return Err(ChunkError::InvalidMagic);
        }

        let version = input.read_u16()?;
        if version != CHUNK_FORMAT_VERSION {
            return Err(ChunkError::UnsupportedVersion(version));
        }

        let coord = ChunkCoord {
            x: isize::try_from(input.read_i64()?).map_err(|_| ChunkError::CoordinateOutOfRange)?,
            y: isize::try_from(input.read_i64()?).map_err(|_| ChunkError::CoordinateOutOfRange)?,
        };
        input.coord = coord;

        let mut chunk = Self::new_empty(coord);
        for row in chunk.cont.iter_mut() {
            for path in row {
                let kind = PathKind::from_id(input.read_u8()?)?;
                let payload_len =
                    usize::try_from(input.read_u32()?).map_err(|_| ChunkError::PayloadTooLarge)?;
                let payload = input.read_bytes(payload_len)?.to_vec();
                let mut path_data = LoadingChunk::new(payload, coord);
                *path = kind.load(&mut path_data)?;
                if !path_data.is_finished() {
                    return Err(ChunkError::TrailingData);
                }
            }
        }

        if !input.is_finished() {
            return Err(ChunkError::TrailingData);
        }
        Ok(chunk)
    }
}

#[derive(Clone, Copy)]
pub enum BotState {
    Ok,
    Terminate,
}

/// Defines a struct as qualifying as a path
pub trait Path {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> BotState;
    fn flyover(&mut self, bot: &mut Bot, world: &mut World);
    fn store(&self, out: &mut Vec<u8>);
    fn kind(&self) -> PathKind;
    fn load(from: &mut LoadingChunk) -> Result<Self, ChunkError>
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
    fn from_id(id: u8) -> Result<Self, ChunkError> {
        match id {
            0 => Ok(Self::NoPath),
            1 => Ok(Self::BasicPath),
            _ => Err(ChunkError::UnknownPathKind(id)),
        }
    }

    fn load(self, from: &mut LoadingChunk) -> Result<Box<dyn Path>, ChunkError> {
        match self {
            Self::NoPath => Ok(Box::new(NoPath::load(from)?)),
            Self::BasicPath => Ok(Box::new(BasicPath::load(from)?)),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ChunkError {
    InvalidMagic,
    UnsupportedVersion(u16),
    UnknownPathKind(u8),
    Truncated,
    TrailingData,
    CoordinateOutOfRange,
    PayloadTooLarge,
}

impl std::fmt::Display for ChunkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMagic => write!(f, "invalid chunk file magic"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported chunk format version {version}")
            }
            Self::UnknownPathKind(id) => write!(f, "unknown path kind ID {id}"),
            Self::Truncated => write!(f, "chunk data is truncated"),
            Self::TrailingData => write!(f, "chunk data contains unexpected trailing bytes"),
            Self::CoordinateOutOfRange => write!(f, "chunk coordinate is out of range"),
            Self::PayloadTooLarge => write!(f, "path payload exceeds the format limit"),
        }
    }
}

impl std::error::Error for ChunkError {}

pub struct LoadingChunk {
    data: Vec<u8>,
    pos: usize,
    coord: ChunkCoord,
}

impl LoadingChunk {
    fn new(data: Vec<u8>, coord: ChunkCoord) -> Self {
        Self {
            data,
            pos: 0,
            coord,
        }
    }

    pub fn coord(&self) -> ChunkCoord {
        self.coord
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<&[u8], ChunkError> {
        let end = self.pos.checked_add(len).ok_or(ChunkError::Truncated)?;
        let bytes = self.data.get(self.pos..end).ok_or(ChunkError::Truncated)?;
        self.pos = end;
        Ok(bytes)
    }

    pub fn read_u8(&mut self) -> Result<u8, ChunkError> {
        Ok(self.read_bytes(1)?[0])
    }

    pub fn read_u16(&mut self) -> Result<u16, ChunkError> {
        Ok(u16::from_le_bytes(
            self.read_bytes(2)?.try_into().expect("two bytes were read"),
        ))
    }

    pub fn read_u32(&mut self) -> Result<u32, ChunkError> {
        Ok(u32::from_le_bytes(
            self.read_bytes(4)?
                .try_into()
                .expect("four bytes were read"),
        ))
    }

    pub fn read_i64(&mut self) -> Result<i64, ChunkError> {
        Ok(i64::from_le_bytes(
            self.read_bytes(8)?
                .try_into()
                .expect("eight bytes were read"),
        ))
    }

    fn is_finished(&self) -> bool {
        self.pos == self.data.len()
    }
}

/// Stores a single bot and all the information it needs to function
pub struct Bot {
    pos: BotPos,
    fly_height: usize,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct ChunkCoord {
    pub x: isize,
    pub y: isize,
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
    fn forward(&mut self, world: &mut World) {
        let (dir_x, dir_y) = match self.facing {
            0 => (1, 0),
            1 => (0, 1),
            2 => (-1, 0),
            3 => (0, -1),
            _ => panic!("This should be unreachable"),
        };
        self.shift(world, dir_x, dir_y);
    }
    fn shift(&mut self, world: &mut World, dir_x: isize, dir_y: isize) {
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
            Equal => {} // If it is equal, that means that this is the bot that requested that the chunk that caused the update be loaded,
            // so it will be passed the id anyway, so nothing needs to happen here.
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
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            bots: Vec::new(),
        }
    }
    fn request_chunk_id(&mut self, pos: ChunkCoord) -> usize {
        match self.chunks.binary_search_by_key(&pos, |chunk| chunk.coord) {
            Ok(t) => t,
            Err(i) => {
                panic!("We don't have a way to try to load things yet")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::BasicPath;

    #[test]
    fn chunk_round_trip_preserves_coordinate_and_path_kinds() {
        let coord = ChunkCoord { x: -12, y: 34 };
        let mut chunk = Chunk::new_empty(coord);
        chunk.cont[3][7] = Box::new(BasicPath());

        let bytes = chunk.to_bytes().unwrap();
        let loaded = Chunk::from_bytes(&bytes).unwrap();

        assert_eq!(loaded.coord(), coord);
        assert_eq!(loaded.cont[3][7].kind(), PathKind::BasicPath);
        assert_eq!(loaded.cont[0][0].kind(), PathKind::NoPath);
        assert_eq!(loaded.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn chunk_load_rejects_unknown_path_ids() {
        let mut bytes = Chunk::new_empty(ChunkCoord { x: 0, y: 0 })
            .to_bytes()
            .unwrap();
        bytes[22] = 255;

        assert!(matches!(
            Chunk::from_bytes(&bytes),
            Err(ChunkError::UnknownPathKind(255))
        ));
    }

    #[test]
    fn chunk_load_rejects_unsupported_versions() {
        let mut bytes = Chunk::new_empty(ChunkCoord { x: 0, y: 0 })
            .to_bytes()
            .unwrap();
        bytes[4..6].copy_from_slice(&2_u16.to_le_bytes());

        assert!(matches!(
            Chunk::from_bytes(&bytes),
            Err(ChunkError::UnsupportedVersion(2))
        ));
    }

    #[test]
    fn chunk_load_rejects_truncated_and_trailing_data() {
        let mut bytes = Chunk::new_empty(ChunkCoord { x: 0, y: 0 })
            .to_bytes()
            .unwrap();
        bytes.pop();
        assert!(matches!(
            Chunk::from_bytes(&bytes),
            Err(ChunkError::Truncated)
        ));

        let mut bytes = Chunk::new_empty(ChunkCoord { x: 0, y: 0 })
            .to_bytes()
            .unwrap();
        bytes.push(0);
        assert!(matches!(
            Chunk::from_bytes(&bytes),
            Err(ChunkError::TrailingData)
        ));
    }

    #[test]
    fn chunk_load_rejects_unconsumed_path_payload() {
        let mut bytes = Chunk::new_empty(ChunkCoord { x: 0, y: 0 })
            .to_bytes()
            .unwrap();
        bytes[23..27].copy_from_slice(&1_u32.to_le_bytes());
        bytes.insert(27, 0);

        assert!(matches!(
            Chunk::from_bytes(&bytes),
            Err(ChunkError::TrailingData)
        ));
    }
}
