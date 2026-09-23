pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

use anchor_spl::token_2022::spl_token_2022::solana_zk_sdk::encryption::AE_CIPHERTEXT_LEN;
pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("23p1sp6EyHEvK1S8cS9AFPtLQDbsKKUzgXXtB6Ys7TUC");

#[program]
pub mod confidential_22 {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        transfer_fee_bps: u16,
        maximum_fee: u64,
        decimals: u8,
        elgamal_pubkey: [u8; 32],
    ) -> Result<()> {
        ctx.accounts.process_ix(
            transfer_fee_bps,
            maximum_fee,
            decimals,
            &ctx.bumps,
            elgamal_pubkey,
        )
    }

    pub fn transfer(ctx: Context<Transfer>, amount: u64) -> Result<()> {
        ctx.accounts.process_ix(amount)
    }

    pub fn unfreeze(ctx: Context<Unfreeze>) -> Result<()> {
        ctx.accounts.process_ix()
    }

    pub fn deposit_confidential(ctx: Context<DepositConfidential>, amount: u64) -> Result<()> {
        ctx.accounts.process_ix(amount)
    }

    pub fn apply_pending_balance(
        ctx: Context<ApplyPendingBalance>,
        expected_pending_balance_credit_counter: u64,
        new_decryptable_available_balance: [u8; AE_CIPHERTEXT_LEN],
    ) -> Result<()> {
        ctx.accounts.process_ix(
            expected_pending_balance_credit_counter,
            new_decryptable_available_balance,
        )
    }

    pub fn approve_account(ctx: Context<ApproveAccount>) -> Result<()> {
        ctx.accounts.process_ix()
    }
}
