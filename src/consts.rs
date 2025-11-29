//! Common constants used across the AMM swap client.

/// The Solana native token mint (wrapped SOL).
pub const SOL_MINT: &str = "So11111111111111111111111111111111111111112";

/// Numerator for Raydium liquidity fee (25 / 10_000 = 0.25%).
pub const LIQUIDITY_FEES_NUMERATOR: u64 = 25;

/// Denominator for Raydium liquidity fee.
pub const LIQUIDITY_FEES_DENOMINATOR: u64 = 10000;

/// Program ID for Raydium AMM V4.
pub const AMM_V4: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";

pub const CLMM: &str = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";

pub const CPMM: &str = "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C";

pub const ADMIN: &str = "GThUX1Atko4tqhN2NaiTazWSeFWMuiUvfFnyJyUghFMJ";
pub const OPEN_BOOK: &str = "srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX";
