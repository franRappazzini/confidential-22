use anchor_lang::prelude::*;
use anchor_spl::token_2022::{thaw_account, ThawAccount, Token2022};

use crate::{constants::*, state::Config};

#[derive(Accounts)]
pub struct Unfreeze<'info> {
    #[account(mut, address = config.authority)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,

    /// CHECK: use unchecked account becasuse some extensions
    #[account(address = config.mint)]
    pub mint: UncheckedAccount<'info>,

    /// CHECK: use unchecked account becasuse some extensions
    #[account(mut, owner = token_program.key())]
    pub ata: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> Unfreeze<'info> {
    pub fn process_ix(&mut self) -> Result<()> {
        // kyc procces successfully

        thaw_account(CpiContext::new(
            self.token_program.key(),
            ThawAccount {
                account: self.ata.to_account_info(),
                authority: self.authority.to_account_info(),
                mint: self.mint.to_account_info(),
            },
        ))
    }
}
