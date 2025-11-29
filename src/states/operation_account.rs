use anchor_lang::prelude::*;
use std::collections::HashSet;

pub const OPERATION_SEED: &str = "operation";
pub const OPERATION_SIZE_USIZE: usize = 10;
pub const WHITE_MINT_SIZE_USIZE: usize = 100;

/// Holds the current owner of the factory
#[account(zero_copy(unsafe))]
#[repr(C, packed)]
#[derive(Debug)]
pub struct OperationState {
    /// Bump to identify PDA
    pub bump: u8,
    /// Address of the operation owner
    pub operation_owners: [Pubkey; OPERATION_SIZE_USIZE],
    /// The mint address of whitelist to emmit reward
    pub whitelist_mints: [Pubkey; WHITE_MINT_SIZE_USIZE],
}

impl OperationState {
    pub const LEN: usize = 8 + 1 + 32 * OPERATION_SIZE_USIZE + 32 * WHITE_MINT_SIZE_USIZE;
    pub fn initialize(&mut self, bump: u8) {
        self.bump = bump;
        self.operation_owners = [Pubkey::default(); OPERATION_SIZE_USIZE];
        self.whitelist_mints = [Pubkey::default(); WHITE_MINT_SIZE_USIZE];
    }

    pub fn validate_operation_owner(&self, owner: Pubkey) -> bool {
        owner != Pubkey::default() && self.operation_owners.contains(&owner)
    }

    pub fn validate_whitelist_mint(&self, mint: Pubkey) -> bool {
        mint != Pubkey::default() && self.whitelist_mints.contains(&mint)
    }

    pub fn update_operation_owner(&mut self, keys: Vec<Pubkey>) {
        let mut operation_owners = self.operation_owners.to_vec();
        operation_owners.extend(keys.as_slice().iter());
        operation_owners.retain(|&item| item != Pubkey::default());
        let owners_set: HashSet<Pubkey> = HashSet::from_iter(operation_owners.iter().cloned());
        let mut updated_owner: Vec<Pubkey> = owners_set.into_iter().collect();
        updated_owner.sort_by(|a, b| a.cmp(b));
        // clear
        self.operation_owners = [Pubkey::default(); OPERATION_SIZE_USIZE];
        // update
        self.operation_owners[0..updated_owner.len()].copy_from_slice(updated_owner.as_slice());
    }

    pub fn remove_operation_owner(&mut self, keys: Vec<Pubkey>) {
        let mut operation_owners = self.operation_owners.to_vec();
        // remove keys from operation_owners
        operation_owners.retain(|x| !keys.contains(&x));
        // clear
        self.operation_owners = [Pubkey::default(); OPERATION_SIZE_USIZE];
        // update
        self.operation_owners[0..operation_owners.len()]
            .copy_from_slice(operation_owners.as_slice());
    }

    pub fn update_whitelist_mint(&mut self, keys: Vec<Pubkey>) {
        let mut whitelist_mints = self.whitelist_mints.to_vec();
        whitelist_mints.extend(keys.as_slice().iter());
        whitelist_mints.retain(|&item| item != Pubkey::default());
        let owners_set: HashSet<Pubkey> = HashSet::from_iter(whitelist_mints.iter().cloned());
        let updated_mints: Vec<Pubkey> = owners_set.into_iter().collect();
        // clear
        self.whitelist_mints = [Pubkey::default(); WHITE_MINT_SIZE_USIZE];
        // update
        self.whitelist_mints[0..updated_mints.len()].copy_from_slice(updated_mints.as_slice());
    }

    pub fn remove_whitelist_mint(&mut self, keys: Vec<Pubkey>) {
        let mut whitelist_mints = self.whitelist_mints.to_vec();
        // remove keys from whitelist_mint
        whitelist_mints.retain(|x| !keys.contains(&x));
        // clear
        self.whitelist_mints = [Pubkey::default(); WHITE_MINT_SIZE_USIZE];
        // update
        self.whitelist_mints[0..whitelist_mints.len()].copy_from_slice(whitelist_mints.as_slice());
    }
}
