//! Fixed-size local level-update frames. Token keys belong to a caller-owned session map.
use crate::{Result, book::Side, polygon::PRICE_SCALE};
use serde::Serialize;
pub const FRAME_BYTES: usize = 40;
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct LevelFrame {
    pub token_key: u32,
    pub side: Side,
    pub price_micro: u32,
    pub shares: u32,
    pub sequence: u64,
    pub time_ms: u64,
}
impl LevelFrame {
    fn validate(&self) -> Result<()> {
        if self.token_key == 0
            || self.price_micro == 0
            || self.price_micro >= PRICE_SCALE
            || self.sequence == 0
        {
            return Err("invalid_level_frame".into());
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<[u8; FRAME_BYTES]> {
        self.validate()?;
        let mut b = [0u8; FRAME_BYTES];
        b[..4].copy_from_slice(b"HFT1");
        b[4] = 1;
        b[5] = 1;
        b[6] = match self.side {
            Side::Buy => 0,
            Side::Sell => 1,
        };
        b[8..12].copy_from_slice(&self.token_key.to_le_bytes());
        b[12..16].copy_from_slice(&self.price_micro.to_le_bytes());
        b[16..20].copy_from_slice(&self.shares.to_le_bytes());
        b[24..32].copy_from_slice(&self.sequence.to_le_bytes());
        b[32..40].copy_from_slice(&self.time_ms.to_le_bytes());
        Ok(b)
    }
    pub fn decode(b: &[u8]) -> Result<Self> {
        if b.len() != FRAME_BYTES
            || &b[..4] != b"HFT1"
            || b[4] != 1
            || b[5] != 1
            || b[7] != 0
            || b[20..24] != [0; 4]
        {
            return Err("invalid_frame_header_or_length".into());
        }
        let side = match b[6] {
            0 => Side::Buy,
            1 => Side::Sell,
            _ => return Err("invalid_frame_side".into()),
        };
        let frame = Self {
            token_key: u32::from_le_bytes(b[8..12].try_into().unwrap()),
            side,
            price_micro: u32::from_le_bytes(b[12..16].try_into().unwrap()),
            shares: u32::from_le_bytes(b[16..20].try_into().unwrap()),
            sequence: u64::from_le_bytes(b[24..32].try_into().unwrap()),
            time_ms: u64::from_le_bytes(b[32..40].try_into().unwrap()),
        };
        frame.validate()?;
        Ok(frame)
    }
}
