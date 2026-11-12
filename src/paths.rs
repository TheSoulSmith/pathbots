use crate::core::*;

#[derive(Clone, Copy)]
pub struct NoPath();

impl Path for NoPath {
    fn bot(&mut self, _bot: &mut Bot, _world: &mut World) -> BotState {
        BotState::Terminate
    }
    fn flyover(&mut self, _bot: &mut Bot, _world: &mut World) {}
    fn store(&self, _out: &mut Vec<u8>) {}
    fn kind(&self) -> PathKind {
        PathKind::NoPath
    }
    fn load(_from: &mut LoadingChunk) -> Result<Self, ChunkError>
    where
        Self: Sized,
    {
        Ok(Self())
    }
}

#[derive(Clone)]
pub struct BasicPath();

impl Path for BasicPath {
    fn bot(&mut self, _bot: &mut Bot, _world: &mut World) -> BotState {
        BotState::Ok
    }
    fn flyover(&mut self, _bot: &mut Bot, _world: &mut World) {}
    fn store(&self, _out: &mut Vec<u8>) {}
    fn kind(&self) -> PathKind {
        PathKind::BasicPath
    }
    fn load(_from: &mut LoadingChunk) -> Result<Self, ChunkError>
    where
        Self: Sized,
    {
        Ok(Self())
    }
}
