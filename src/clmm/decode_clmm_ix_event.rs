use crate::common::InstructionDecodeType;
use anchor_client::ClientError;
use anyhow::Result;

/// Stubbed CLMM instruction decoder.
///
/// The original implementation depended on concrete discriminator
/// layouts and multiple Solana SDK versions, which conflicted with
/// this client's dependency graph. For now, this helper is a no-op
/// that just validates basic input shape.
pub fn handle_program_instruction(
    instr_data: &str,
    _decode_type: InstructionDecodeType,
) -> Result<(), ClientError> {
    if instr_data.is_empty() {
        println!("Empty instruction data");
    }
    Ok(())
}

