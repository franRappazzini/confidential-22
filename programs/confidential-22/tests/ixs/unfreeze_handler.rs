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

pub fn create_unfreeze_ix(
    authority: &Keypair,
    config: Address,
    mint: &Keypair,
    ata: Address,
) -> Instruction {
    Instruction::new_with_bytes(
        confidential_22::ID,
        &confidential_22::instruction::Unfreeze {}.data(),
        confidential_22::accounts::Unfreeze {
            authority: authority.pubkey(),
            config,
            mint: mint.pubkey(),
            ata,
            token_program: token_2022::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}
