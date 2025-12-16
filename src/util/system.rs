use anchor_lang::prelude::*;

pub fn get_recent_epoch() -> Result<u64> {
    Ok(Clock::get()?.epoch)
}
