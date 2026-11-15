use super::PathKind;
use crate::core::*;

#[derive(Clone)]
pub(crate) struct SpeedUp();

impl Path for SpeedUp {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        bot.change_action_time(-1);
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, _out: &mut Vec<u8>) {}
    fn kind(&self) -> PathKind {
        PathKind::SpeedUp
    }
    fn load(_from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self())
    }
}

#[derive(Clone)]
pub(crate) struct SlowDown();

impl Path for SlowDown {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        bot.change_action_time(1);
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, _out: &mut Vec<u8>) {}
    fn kind(&self) -> PathKind {
        PathKind::SlowDown
    }
    fn load(_from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self())
    }
}
