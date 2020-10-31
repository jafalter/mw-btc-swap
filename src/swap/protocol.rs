use std::net::TcpStream;
use crate::SwapSlate;

/// Runs the mimblewimble side of the setup phase of the atomic swap
/// 
/// # Arguments
/// 
/// * `slate` reference to the atomic swap slate
pub fn setup_phase_swap_mw(slate : &mut SwapSlate, stream : &mut TcpStream) -> Result<SwapSlate, &'static str> {
    Err("Not implemented")
}

/// Runs the bitcoin side of the setup phase of the atomic swap
/// 
/// # Arguments
/// 
/// * `slate` reference to the atomic swap slate
pub fn setup_phase_swap_btc(slate : &mut SwapSlate, stream : &mut TcpStream) -> Result<SwapSlate, &'static str> {
    Err("Not implemented")
}