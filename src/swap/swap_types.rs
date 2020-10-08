use crate::enums::SwapStatus;
use crate::commands::cmd_types::Offer;

pub struct SwapState {
    pub status : SwapStatus,
    pub offer : Offer
}