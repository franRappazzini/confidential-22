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
pub struct DepositConfidential<'info> {
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

impl<'info> DepositConfidential<'info> {
    pub fn process_ix(&mut self, amount: u64) -> Result<()> {
        let account_info = self.mint.to_account_info();
        let data = account_info.try_borrow_data()?;
        let state = StateWithExtensions::<Mint>::unpack(&data)?;

        let decimals = state.base.decimals;

        let ix = confidential_transfer::instruction::deposit(
            &self.token_program.key(),
            &self.ata.key(),
            &self.mint.key(),
            amount,
            decimals,
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
