use deku::prelude::*;

#[derive(Debug, PartialEq, DekuRead, DekuWrite, Clone)]
#[deku(type = "u8")]
pub enum Message {
  #[deku(id = "0")]
  SpawnFood(f32, f32),
  #[deku(id = "1")]
  NewPlayer(f32, f32),
  #[deku(id = "2")]
  MovePlayer(f32, f32),
}

