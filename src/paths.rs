use crate::core::*;

#[derive(Clone, Copy)]
pub struct NoPath();

impl Path for NoPath {
    fn bot(&mut self, _bot: &mut Bot, _world: &mut World) -> Result<BotState, String> {
        Ok(BotState::Terminate)
    }
    fn store(&self, _out: &mut Vec<u8>) {}
    fn kind(&self) -> PathKind {
        PathKind::NoPath
    }
    fn load(_from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self())
    }
}

#[derive(Clone)]
pub struct BasicPath();

impl Path for BasicPath {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, _out: &mut Vec<u8>) {}
    fn kind(&self) -> PathKind {
        PathKind::BasicPath
    }
    fn load(_from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self())
    }
}
