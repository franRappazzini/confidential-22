use anchor_lang::{prelude::*, solana_program::program::invoke};
use anchor_spl::{
    token_2022::{
        spl_token_2022::{
            extension::{
                confidential_transfer, confidential_transfer_fee, transfer_fee::TransferFeeConfig,
                BaseStateWithExtensions, StateWithExtensions,
            },
            solana_zk_sdk::encryption::{
                pod::{auth_encryption::PodAeCiphertext, elgamal::PodElGamalCiphertext},
                AE_CIPHERTEXT_LEN, ELGAMAL_CIPHERTEXT_LEN,
            },
            state::Mint,
        },
        Token2022,
    },
    token_interface::{transfer_checked_with_fee, TransferCheckedWithFee},
};

use crate::{constants::*, state::Config};

#[derive(Accounts)]
pub struct Transfer<'info> {
    #[account(mut)]
    pub from: Signer<'info>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,

    /// CHECK: use unchecked account becasuse some extensions
    #[account(address = config.mint)]
    pub mint: UncheckedAccount<'info>,

    /// CHECK: validated by token-2022
    #[account(mut, owner = token_program.key())]
    pub from_ata: UncheckedAccount<'info>,

    /// CHECK: validated by token-2022
    #[account(mut, owner = token_program.key())]
    pub to_ata: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> Transfer<'info> {
    pub fn process_ix(
        &mut self,
        amount: u64,
        // new_source_decryptable_available_balance: [u8; AE_CIPHERTEXT_LEN],
        // transfer_amount_auditor_ciphertext_lo: [u8; ELGAMAL_CIPHERTEXT_LEN],
        // transfer_amount_auditor_ciphertext_hi: [u8; ELGAMAL_CIPHERTEXT_LEN],
        // equality_proof_data_location: Pubkey,
        // transfer_amount_ciphertext_validity_proof_data_location: Pubkey,
        // fee_sigma_proof_data_location: Pubkey,
        // fee_ciphertext_validity_proof_data_location: Pubkey,
        // range_proof_data_location: Pubkey,
    ) -> Result<()> {
        let account_info = self.mint.to_account_info();
        let data = account_info.try_borrow_data()?;
        let state = StateWithExtensions::<Mint>::unpack(&data)?;

        // Available from the typed account, no extension awareness needed.
        let decimals = state.base.decimals;

        let transfer_fee_config = state.get_extension::<TransferFeeConfig>()?;

        // Calcular la tarifa basada en el epoch actual de la red
        let clock = Clock::get()?;
        let current_epoch = clock.epoch;
        let fee = transfer_fee_config
            .calculate_epoch_fee(current_epoch, amount)
            .ok_or(ProgramError::InvalidArgument)?;

        // transfer_checked_with_fee(
        //     CpiContext::new(
        //     self.token_program.key(),
        //     TransferCheckedWithFee {
        //         token_program_id: self.token_program.to_account_info(),
        //         source: self.from_ata.to_account_info(),
        //         mint: self.mint.to_account_info(),
        //         destination: self.to_ata.to_account_info(),
        //         authority: self.from.to_account_info(),
        //     },
        // ),
        // amount,
        // decimals,
        // fee,
        // )

        // let ix = confidential_transfer::instruction::transfer_with_fee(
        //     &self.token_program.key(),
        //     &self.from_ata.key(),
        //     &self.mint.key(),
        //     &self.to_ata.key(),
        //     &PodAeCiphertext::from(new_source_decryptable_available_balance),
        //     &PodElGamalCiphertext::from(transfer_amount_auditor_ciphertext_lo),
        //     &PodElGamalCiphertext::from(transfer_amount_auditor_ciphertext_hi),
        //     &self.from.key(),
        //     &[],
        //     &equality_proof_data_location,
        //     &transfer_amount_ciphertext_validity_proof_data_location,
        //     &fee_sigma_proof_data_location,
        //     &fee_ciphertext_validity_proof_data_location,
        //     &range_proof_data_location,
        // )?;

        Ok(())
    }
}
