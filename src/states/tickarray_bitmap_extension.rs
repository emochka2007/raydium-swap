use crate::consts::CLMM;
use crate::libraries::error::ErrorCode;
use crate::libraries::{
    U512,
    tick_array_bit_map::{
        TICK_ARRAY_BITMAP_SIZE, TickArryBitmap, get_bitmap_tick_boundary,
        max_tick_in_tickarray_bitmap,
    },
    tick_math,
};
use crate::states::{POOL_TICK_ARRAY_BITMAP_SEED, TickArrayState};
use anchor_lang::prelude::*;
use std::ops::BitXor;

const EXTENSION_TICKARRAY_BITMAP_SIZE: usize = 14;

#[account(zero_copy(unsafe))]
#[repr(C, packed)]
#[derive(Debug)]
pub struct TickArrayBitmapExtension {
    pub pool_id: Pubkey,
    /// Packed initialized tick array state for start_tick_index is positive
    pub positive_tick_array_bitmap: [[u64; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
    /// Packed initialized tick array state for start_tick_index is negitive
    pub negative_tick_array_bitmap: [[u64; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
}

impl Default for TickArrayBitmapExtension {
    #[inline]
    fn default() -> TickArrayBitmapExtension {
        TickArrayBitmapExtension {
            pool_id: Pubkey::default(),
            positive_tick_array_bitmap: [[0; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
            negative_tick_array_bitmap: [[0; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE],
        }
    }
}

impl TickArrayBitmapExtension {
    pub const LEN: usize = 8 + 32 + 64 * EXTENSION_TICKARRAY_BITMAP_SIZE * 2;

    pub fn initialize(&mut self, pool_id: Pubkey) {
        self.pool_id = pool_id;
        self.positive_tick_array_bitmap = [[0; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE];
        self.negative_tick_array_bitmap = [[0; 8]; EXTENSION_TICKARRAY_BITMAP_SIZE];
    }

    pub fn key(pool_id: Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(), pool_id.as_ref()],
            &Pubkey::from_str_const(CLMM),
        )
        .0
    }

    fn get_bitmap_offset(tick_index: i32, tick_spacing: u16) -> Result<usize> {
        require!(
            TickArrayState::check_is_valid_start_index(tick_index, tick_spacing),
            ErrorCode::InvaildTickIndex
        );
        Self::check_extension_boundary(tick_index, tick_spacing)?;
        let ticks_in_one_bitmap = max_tick_in_tickarray_bitmap(tick_spacing);
        let mut offset = tick_index.abs() / ticks_in_one_bitmap - 1;
        if tick_index < 0 && tick_index.abs() % ticks_in_one_bitmap == 0 {
            offset -= 1;
        }
        Ok(offset as usize)
    }

    /// According to the given tick, calculate its corresponding tickarray and then find the bitmap it belongs to.
    fn get_bitmap(&self, tick_index: i32, tick_spacing: u16) -> Result<(usize, TickArryBitmap)> {
        let offset = Self::get_bitmap_offset(tick_index, tick_spacing)?;
        if tick_index < 0 {
            Ok((offset, self.negative_tick_array_bitmap[offset]))
        } else {
            Ok((offset, self.positive_tick_array_bitmap[offset]))
        }
    }

    /// Check if the tick in tick array bitmap extension
    pub fn check_extension_boundary(tick_index: i32, tick_spacing: u16) -> Result<()> {
        let positive_tick_boundary = max_tick_in_tickarray_bitmap(tick_spacing);
        let negative_tick_boundary = -positive_tick_boundary;
        require_gt!(tick_math::MAX_TICK, positive_tick_boundary);
        require_gt!(negative_tick_boundary, tick_math::MIN_TICK);
        if tick_index >= negative_tick_boundary && tick_index < positive_tick_boundary {
            return err!(ErrorCode::InvalidTickArrayBoundary);
        }
        Ok(())
    }

    /// Check if the tick array is initialized
    pub fn check_tick_array_is_initialized(
        &self,
        tick_array_start_index: i32,
        tick_spacing: u16,
    ) -> Result<(bool, i32)> {
        let (_, tickarray_bitmap) = self.get_bitmap(tick_array_start_index, tick_spacing)?;

        let tick_array_offset_in_bitmap =
            Self::tick_array_offset_in_bitmap(tick_array_start_index, tick_spacing);

        if U512(tickarray_bitmap).bit(tick_array_offset_in_bitmap as usize) {
            return Ok((true, tick_array_start_index));
        }
        Ok((false, tick_array_start_index))
    }

    /// Flip the value of tick in the bitmap.
    pub fn flip_tick_array_bit(
        &mut self,
        tick_array_start_index: i32,
        tick_spacing: u16,
    ) -> Result<()> {
        let (offset, tick_array_bitmap) = self.get_bitmap(tick_array_start_index, tick_spacing)?;
        let tick_array_offset_in_bitmap =
            Self::tick_array_offset_in_bitmap(tick_array_start_index, tick_spacing);
        let tick_array_bitmap = U512(tick_array_bitmap);
        let mask = U512::one() << tick_array_offset_in_bitmap;
        if tick_array_start_index < 0 {
            self.negative_tick_array_bitmap[offset] = tick_array_bitmap.bitxor(mask).0;
        } else {
            self.positive_tick_array_bitmap[offset] = tick_array_bitmap.bitxor(mask).0;
        }
        Ok(())
    }

    /// Search for the first initialized bit in bitmap according to the direction, if found return ture and the tick array start index,
    /// if not, return false and tick boundary index
    pub fn next_initialized_tick_array_from_one_bitmap(
        &self,
        last_tick_array_start_index: i32,
        tick_spacing: u16,
        zero_for_one: bool,
    ) -> Result<(bool, i32)> {
        let multiplier = TickArrayState::tick_count(tick_spacing);
        let next_tick_array_start_index = if zero_for_one {
            last_tick_array_start_index - multiplier
        } else {
            last_tick_array_start_index + multiplier
        };
        let min_tick_array_start_index =
            TickArrayState::get_array_start_index(tick_math::MIN_TICK, tick_spacing);
        let max_tick_array_start_index =
            TickArrayState::get_array_start_index(tick_math::MAX_TICK, tick_spacing);

        if next_tick_array_start_index < min_tick_array_start_index
            || next_tick_array_start_index > max_tick_array_start_index
        {
            return Ok((false, next_tick_array_start_index));
        }

        let (_, tickarray_bitmap) = self.get_bitmap(next_tick_array_start_index, tick_spacing)?;

        Ok(Self::next_initialized_tick_array_in_bitmap(
            tickarray_bitmap,
            next_tick_array_start_index,
            tick_spacing,
            zero_for_one,
        ))
    }

    pub fn next_initialized_tick_array_in_bitmap(
        tickarray_bitmap: TickArryBitmap,
        next_tick_array_start_index: i32,
        tick_spacing: u16,
        zero_for_one: bool,
    ) -> (bool, i32) {
        let (bitmap_min_tick_boundary, bitmap_max_tick_boundary) =
            get_bitmap_tick_boundary(next_tick_array_start_index, tick_spacing);

        let tick_array_offset_in_bitmap =
            Self::tick_array_offset_in_bitmap(next_tick_array_start_index, tick_spacing);
        if zero_for_one {
            // tick from upper to lower
            // find from higher bits to lower bits
            let offset_bit_map = U512(tickarray_bitmap)
                << (TICK_ARRAY_BITMAP_SIZE - 1 - tick_array_offset_in_bitmap);

            let next_bit = if offset_bit_map.is_zero() {
                None
            } else {
                Some(u16::try_from(offset_bit_map.leading_zeros()).unwrap())
            };

            if let Some(next_bit) = next_bit {
                let next_array_start_index = next_tick_array_start_index
                    - i32::from(next_bit) * TickArrayState::tick_count(tick_spacing);
                (true, next_array_start_index)
            } else {
                // not found til to the end
                (false, bitmap_min_tick_boundary)
            }
        } else {
            // tick from lower to upper
            // find from lower bits to highter bits
            let offset_bit_map = U512(tickarray_bitmap) >> tick_array_offset_in_bitmap;

            let next_bit = if offset_bit_map.is_zero() {
                None
            } else {
                Some(u16::try_from(offset_bit_map.trailing_zeros()).unwrap())
            };
            if let Some(next_bit) = next_bit {
                let next_array_start_index = next_tick_array_start_index
                    + i32::from(next_bit) * TickArrayState::tick_count(tick_spacing);
                (true, next_array_start_index)
            } else {
                // not found til to the end
                (
                    false,
                    bitmap_max_tick_boundary - TickArrayState::tick_count(tick_spacing),
                )
            }
        }
    }

    pub fn tick_array_offset_in_bitmap(tick_array_start_index: i32, tick_spacing: u16) -> i32 {
        let m = tick_array_start_index.abs() % max_tick_in_tickarray_bitmap(tick_spacing);
        let mut tick_array_offset_in_bitmap = m / TickArrayState::tick_count(tick_spacing);
        if tick_array_start_index < 0 && m != 0 {
            tick_array_offset_in_bitmap = TICK_ARRAY_BITMAP_SIZE - tick_array_offset_in_bitmap;
        }
        tick_array_offset_in_bitmap
    }
}
