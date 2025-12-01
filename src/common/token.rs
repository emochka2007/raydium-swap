use solana_sdk::{instruction::Instruction, pubkey::Pubkey};
use solana_system_interface::instruction::create_account;

pub fn create_ata_token_or_not(
    funding: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    token_program: Option<&Pubkey>,
) -> Vec<Instruction> {
    vec![
        spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            funding,
            owner,
            mint,
            token_program.unwrap_or(&spl_token::id()),
        ),
    ]
}

pub fn create_init_token(
    token: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    funding: &Pubkey,
    lamports: u64,
) -> anyhow::Result<Vec<Instruction>> {
    Ok(vec![
        create_account(
            funding,
            token,
            lamports,
            165, // spl_token::state::Account::LEN
            &spl_token::id(),
        ),
        spl_token::instruction::initialize_account(&spl_token::id(), token, mint, owner)?,
    ])
}

pub fn create_init_mint(
    funding: &Pubkey,
    mint: &Pubkey,
    mint_authority: &Pubkey,
    decimals: u8,
    lamports: u64,
) -> anyhow::Result<Vec<Instruction>> {
    Ok(vec![
        create_account(
            funding,
            mint,
            lamports,
            82, // spl_token::state::Mint::LEN
            &spl_token::id(),
        ),
        spl_token::instruction::initialize_mint(
            &spl_token::id(),
            mint,
            mint_authority,
            None,
            decimals,
        )?,
    ])
}

pub fn mint_to(
    mint: &Pubkey,
    to_token: &Pubkey,
    mint_authority: &Pubkey,
    token_program: Option<&Pubkey>,
    amount: u64,
) -> Vec<Instruction> {
    // Not used by the high-level client; left as a no-op
    // to avoid Solana SDK version conflicts.
    let _ = (mint, to_token, mint_authority, token_program, amount);
    Vec::new()
}

pub fn transfer_to(
    from: &Pubkey,
    to: &Pubkey,
    from_authority: &Pubkey,
    token_program: Option<&Pubkey>,
    amount: u64,
) -> Vec<Instruction> {
    vec![
        spl_token::instruction::transfer(
            token_program.unwrap_or(&spl_token::id()),
            from,
            to,
            from_authority,
            &[],
            amount,
        )
        .unwrap(),
    ]
}

pub fn close_spl_account(
    close_account: &Pubkey,
    destination: &Pubkey,
    close_authority: &Pubkey,
    token_program: Option<&Pubkey>,
) -> Vec<Instruction> {
    // Not used by the high-level client; left as a no-op
    // to avoid Solana SDK version conflicts.
    let _ = (close_account, destination, close_authority, token_program);
    Vec::new()
}

pub fn wrap_sol_instructions(from: &Pubkey, to: &Pubkey, amount: u64) -> Vec<Instruction> {
    // Not used by the high-level client; left as a no-op
    // to avoid Solana SDK version conflicts.
    let _ = (from, to, amount);
    Vec::new()
}
