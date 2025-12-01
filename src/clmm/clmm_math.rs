use anyhow::anyhow;

pub const Q64: u128 = (u64::MAX as u128) + 1; // 2^64
pub const RESOLUTION: u8 = 64;

pub fn multiplier(decimals: u8) -> anyhow::Result<f64> {
    let multiplier = (10_i32)
        .checked_pow(decimals.into())
        .ok_or(anyhow!("Error in checked_pow multiplier"))? as f64;
    Ok(multiplier)
}
pub fn price_to_x64(price: f64) -> u128 {
    (price * Q64 as f64) as u128
}

pub fn from_x64_price(price: u128) -> f64 {
    price as f64 / Q64 as f64
}

pub fn price_to_sqrt_price_x64(price: f64, decimals_0: u8, decimals_1: u8) -> anyhow::Result<u128> {
    let price_with_decimals = price * multiplier(decimals_1)? / multiplier(decimals_0)?;
    Ok(price_to_x64(price_with_decimals.sqrt()))
}

pub fn sqrt_price_x64_to_price(price: u128, decimals_0: u8, decimals_1: u8) -> anyhow::Result<f64> {
    Ok(from_x64_price(price).powi(2) * multiplier(decimals_0)? / multiplier(decimals_1)?)
}

pub fn tick_with_spacing(tick: i32, tick_spacing: i32) -> i32 {
    let mut compressed = tick / tick_spacing;
    if tick < 0 && tick % tick_spacing != 0 {
        compressed -= 1; // round towards negative infinity
    }
    compressed * tick_spacing
}
