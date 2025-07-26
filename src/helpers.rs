//! Utility functions for environment parsing and keypair handling.

use solana_sdk::signature::Keypair;

/// Parses a string of bytes (`"[150, 12, 123, ...]"`) into a Solana `Keypair`.
///
/// # Panics
///
/// - If any element fails to parse into a `u8`.
/// - If the resulting byte slice cannot be converted into a valid `Keypair`.
///
/// # Examples
///
/// ```
/// let env = "[1,2,3,...]".to_string();
/// let kp = from_bytes_to_key_pair(env);
/// ```
pub fn from_bytes_to_key_pair(env: String) -> Keypair {
    let bytes: Vec<u8> = env
        .trim_matches(&['[', ']'][..])
        .split(',')
        .map(|s| s.trim().parse::<u8>().expect("Error converting to bytes"))
        .collect();
    Keypair::from_bytes(&bytes).expect("Error converting bytes to Keypair")
}
