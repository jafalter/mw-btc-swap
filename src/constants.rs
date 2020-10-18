// theoretical max limit of grin offered to swap in NanoGrin
pub const GRIN_MAX_NANOGRIN : u64 = 10000000 * 1000000000; 
// theoretical max limit of btc offered to swap in Satoshis
pub const BTC_MAX_SATS : u64 = 21000000 * 100000000;
// 5 days max timeout (in minutes)
pub const MAX_TIMEOUT : u32 = 60 * 24 * 5;
// Bitcoin avg block time is 10 minutes
pub const BTC_BLOCK_TIME : u32 = 10;
// Grin avg block time is 1 minute
pub const GRIN_BLOCK_TIME :u32 = 1;