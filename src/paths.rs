use crate::core::*;

pub struct NoPath();

impl Path for NoPath {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> BotState {
        BotState::Terminate
    }
    fn flyover(&mut self, bot: &mut Bot, world: &mut World) {}
    fn store(&self, out: &mut Vec<u8>) {}
    fn load(_from: LoadingChunk) -> Self
    where
        Self: Sized,
    {
        Self()
    }
}
