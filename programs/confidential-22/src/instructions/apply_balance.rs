use anchor_lang::{prelude::*, solana_program::program::invoke};
use anchor_spl::token_2022::{
    spl_token_2022::{
        extension::confidential_transfer::{self, DecryptableBalance},
        solana_zk_sdk::encryption::AE_CIPHERTEXT_LEN,
    },
    Token2022,
};

#[derive(Accounts)]
pub struct ApplyPendingBalance<'info> {
    pub authority: Signer<'info>,

    /// CHECK: validated by token-2022
    #[account(mut, owner = token_program.key())]
    pub ata: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token2022>,
}

impl<'info> ApplyPendingBalance<'info> {
    pub fn process_ix(
        &mut self,
        expected_pending_balance_credit_counter: u64,
        new_decryptable_available_balance: [u8; AE_CIPHERTEXT_LEN],
    ) -> Result<()> {
        let balance = DecryptableBalance::from(new_decryptable_available_balance);
        let ix = confidential_transfer::instruction::apply_pending_balance(
            &self.token_program.key(),
            &self.ata.key(),
            expected_pending_balance_credit_counter,
            &balance,
            &self.authority.key(),
            &[],
        )?;
        invoke(
            &ix,
            &[
                self.ata.to_account_info(),
                self.authority.to_account_info(),
                self.token_program.to_account_info(),
            ],
        )?;
        
        Ok(())
    }
}
