use {
    anchor_lang::{
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    anchor_spl::token_2022,
    solana_keypair::Keypair,
    solana_message::Address,
    solana_signer::Signer,
};

pub fn create_initialixe_ix(
    authority: &Keypair,
    config: Address,
    mint: &Keypair,
    decimals: u8,
    transfer_fee_bps: u16,
    maximum_fee: u64,
    fee_authority_pubkey: [u8; 32],
) -> Instruction {
    Instruction::new_with_bytes(
        confidential_22::ID,
        &confidential_22::instruction::Initialize {
            decimals,
            transfer_fee_bps,
            maximum_fee,
            elgamal_pubkey: fee_authority_pubkey,
        }
        .data(),
        confidential_22::accounts::Initialize {
            authority: authority.pubkey(),
            config,
            mint: mint.pubkey(),
            token_program: token_2022::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}
