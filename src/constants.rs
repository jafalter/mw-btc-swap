// conversion rate from btx to sats
pub const BTC_SATS : u64 = 100000000;
// conversion rate from grin to nanogrin
pub const NANO_GRIN : u64 = 1000000000;
// theoretical max limit of grin offered to swap in NanoGrin
pub const GRIN_MAX_NANOGRIN : u64 = 10000000 * NANO_GRIN; 
// theoretical max limit of btc offered to swap in Satoshis
pub const BTC_MAX_SATS : u64 = 21000000 * 100000000;
// 5 days max timeout (in minutes)
pub const MAX_TIMEOUT : u32 = 60 * 24 * 5;
// Bitcoin avg block time is 10 minutes
pub const BTC_BLOCK_TIME : u32 = 10;
// Grin avg block time is 1 minute
pub const GRIN_BLOCK_TIME :u32 = 1;
// If we are running on test net
pub const TEST_NET : bool = true;
// The default value for sequence in bitcoin transactions
pub const FFFFFFFF : u32 = 4294967295;
pub const SIGHASH_ALL : u8 = 0x01;