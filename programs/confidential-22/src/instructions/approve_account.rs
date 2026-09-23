use anchor_lang::{prelude::*, solana_program::program::invoke};
use anchor_spl::token_2022::{
    spl_token_2022::{
        extension::{confidential_transfer, StateWithExtensions},
        state::Mint,
    },
    Token2022,
};

use crate::{Config, CONFIG_SEED};

#[derive(Accounts)]
pub struct ApproveAccount<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,

    /// CHECK: validated by token-2022 and config.mint
    #[account(address = config.mint)]
    pub mint: UncheckedAccount<'info>,

    /// CHECK: validated by token-2022
    #[account(mut, owner = token_program.key())]
    pub ata: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token2022>,
}

impl<'info> ApproveAccount<'info> {
    pub fn process_ix(&mut self) -> Result<()> {
        let ix = confidential_transfer::instruction::approve_account(
            &self.token_program.key(),
            &self.ata.key(),
            &self.mint.key(),
            &self.authority.key(),
            &[],
        )?;
        invoke(
            &ix,
            &[
                self.ata.to_account_info(),
                self.mint.to_account_info(),
                self.authority.to_account_info(),
                self.token_program.to_account_info(),
            ],
        )?;

        Ok(())
    }
}
