use crate::core::*;

macro_rules! path_kinds {
    ($($kind:ident = $id:literal => $path:ident),+ $(,)?) => {
        #[repr(u8)]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        /// Stable on-disk identifiers for path types; never reuse an assigned value.
        pub(crate) enum PathKind {
            $($kind = $id),+
        }

        impl PathKind {
            pub(crate) fn from_id(id: u8) -> Result<Self, String> {
                match id {
                    $($id => Ok(Self::$kind),)+
                    e => Err(format!("Path id {} is not mapped to a path", e)),
                }
            }

            pub(crate) fn load_helper(self, from: &mut LoadingChunk) -> Result<Box<dyn Path>, String> {
                match self {
                    $(Self::$kind => Ok(Box::new($path::load(from)?)),)+
                }
            }

            pub(crate) fn load(self, from: &mut LoadingChunk) -> Result<Box<dyn Path>, String> {
                self.load_helper(from).map_err(|e| format!("{}\nat loading {:?}", e, self))
            }
        }
    };
}

path_kinds! {
    NoPath = 0 => NoPath,
    BasicPath = 1 => BasicPath,
    TurnLeft = 2 => TurnLeft,
    TurnRight = 3 => TurnRight,
    TurnAround = 4 => TurnAround,
    OneWay = 5 => OneWay,
    RandomDir = 6 => RandomDir,
    RandomTurn = 7 => RandomTurn,
    Conveyor = 8 => Conveyor,
    LeftDistributor = 9 => LeftDistributor,
    RightDistributor = 10 => RightDistributor,
    Distributor = 11 => Distributor,
}

#[derive(Clone, Copy)]
pub(crate) struct NoPath();

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
pub(crate) struct BasicPath();

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

#[derive(Clone)]
pub(crate) struct TurnLeft();

impl Path for TurnLeft {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        bot.turn(-1);
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, _out: &mut Vec<u8>) {}
    fn kind(&self) -> PathKind {
        PathKind::TurnLeft
    }
    fn load(_from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self())
    }
}

#[derive(Clone)]
pub(crate) struct TurnRight();

impl Path for TurnRight {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        bot.turn(1);
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, _out: &mut Vec<u8>) {}
    fn kind(&self) -> PathKind {
        PathKind::TurnRight
    }
    fn load(_from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self())
    }
}

#[derive(Clone)]
pub(crate) struct TurnAround();

impl Path for TurnAround {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        bot.turn(2);
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, _out: &mut Vec<u8>) {}
    fn kind(&self) -> PathKind {
        PathKind::TurnAround
    }
    fn load(_from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self())
    }
}

#[derive(Clone)]
pub(crate) struct OneWay(u8);

impl Path for OneWay {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        bot.set_dir(self.0);
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, out: &mut Vec<u8>) {
        out.push(self.0);
    }
    fn kind(&self) -> PathKind {
        PathKind::OneWay
    }
    fn load(from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self(match from.read_u8() {
            Some(t) => t & 3,
            None => return Err(format!("Could not read facing direction")),
        }))
    }
}

#[derive(Clone)]
pub(crate) struct RandomDir();

impl Path for RandomDir {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        bot.set_dir(rand::random_range(0..4));
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, _out: &mut Vec<u8>) {}
    fn kind(&self) -> PathKind {
        PathKind::RandomDir
    }
    fn load(_from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self())
    }
}

#[derive(Clone)]
pub(crate) struct RandomTurn();

impl Path for RandomTurn {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        bot.turn(if rand::random_bool(0.5) { -1 } else { 1 });
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, _out: &mut Vec<u8>) {}
    fn kind(&self) -> PathKind {
        PathKind::RandomTurn
    }
    fn load(_from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self())
    }
}

#[derive(Clone)]
pub(crate) struct Conveyor(u8);

impl Path for Conveyor {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        let (dir_x, dir_y) = match self.0 {
            0 => (1, 0),
            1 => (0, 1),
            2 => (-1, 0),
            3 => (0, -1),
            _ => panic!("This should be unreachable"),
        };
        bot.shift(world, dir_x, dir_y)
            .map_err(|e| format!("{}\nat bot move", e))?;
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, out: &mut Vec<u8>) {
        out.push(self.0);
    }
    fn kind(&self) -> PathKind {
        PathKind::Conveyor
    }
    fn load(from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self(match from.read_u8() {
            Some(t) => t & 3,
            None => return Err(format!("Could not read facing direction")),
        }))
    }
}

#[derive(Clone)]
pub(crate) struct LeftDistributor(u8);

impl Path for LeftDistributor {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        bot.set_dir(self.0);
        self.0 = self.0.wrapping_add_signed(-1) & 3;
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, out: &mut Vec<u8>) {
        out.push(self.0);
    }
    fn kind(&self) -> PathKind {
        PathKind::LeftDistributor
    }
    fn load(from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self(match from.read_u8() {
            Some(t) => t & 3,
            None => return Err(format!("Could not read facing direction")),
        }))
    }
}

#[derive(Clone)]
pub(crate) struct RightDistributor(u8);

impl Path for RightDistributor {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        bot.set_dir(self.0);
        self.0 = self.0.wrapping_add(1) & 3;
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, out: &mut Vec<u8>) {
        out.push(self.0);
    }
    fn kind(&self) -> PathKind {
        PathKind::RightDistributor
    }
    fn load(from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self(match from.read_u8() {
            Some(t) => t & 3,
            None => return Err(format!("Could not read facing direction")),
        }))
    }
}

#[derive(Clone)]
pub(crate) struct Distributor(u8);

impl Path for Distributor {
    fn bot(&mut self, bot: &mut Bot, world: &mut World) -> Result<BotState, String> {
        bot.set_dir(self.0);
        self.0 = self.0.wrapping_add(2) & 3;
        bot.forward(world)?;
        Ok(BotState::Ok)
    }
    fn store(&self, out: &mut Vec<u8>) {
        out.push(self.0);
    }
    fn kind(&self) -> PathKind {
        PathKind::Distributor
    }
    fn load(from: &mut LoadingChunk) -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self(match from.read_u8() {
            Some(t) => t & 3,
            None => return Err(format!("Could not read facing direction")),
        }))
    }
}
