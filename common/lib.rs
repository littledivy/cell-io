use deku::prelude::*;

#[derive(Debug, PartialEq, DekuRead, DekuWrite)]
#[deku(type = "u8")]
pub enum Message {
  #[deku(id = "0")]
  SpawnFood(f32, f32),
}

