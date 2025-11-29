use solana_sdk::{instruction::Instruction, pubkey::Pubkey};
use solana_system_interface::instruction::create_account;

pub fn create_rent_exempt(
    from: &Pubkey,
    to: &Pubkey,
    owner: &Pubkey,
    lamports: u64,
    space: u64,
) -> Vec<Instruction> {
    vec![create_account(from, to, lamports, space, owner)]
}
