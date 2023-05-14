use deku::prelude::*;

// How much of the map is visible to the player at once.
pub const CAMERA_WIDTH: f32 = 1000.0;
pub const CAMERA_HEIGHT: f32 = 1000.0;

// Total map size.
pub const MAP_WIDTH: f32 = 10000.0;
pub const MAP_HEIGHT: f32 = 10000.0;

// Total number of food globules.
pub const MAX_FOOD: usize = 1000;

#[derive(Debug, PartialEq, DekuRead, DekuWrite, Clone)]
#[deku(type = "u8")]
pub enum Message {
  #[deku(id = "0")]
  SpawnFood(f32, f32),
  #[deku(id = "1")]
  // x, y, uid
  NewPlayer(f32, f32, u32),
  #[deku(id = "2")]
  // x, y, uid
  MovePlayer(f32, f32, u32),
  #[deku(id = "3")]
  // x, y, uid
  Start(f32, f32, u32),
}

impl Message {
  pub fn uid(&self) -> Option<u32> {
    match self {
      Message::NewPlayer(_, _, uid) => Some(*uid),
      Message::MovePlayer(_, _, uid) => Some(*uid),
      Message::Start(_, _, uid) => Some(*uid),
      Message::SpawnFood(_, _) => None,
    }
  }
}
