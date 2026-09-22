use anchor_lang::{prelude::*, solana_program::program::invoke, system_program};
use anchor_spl::{
    token_2022::{
        initialize_mint2, initialize_mint_close_authority,
        spl_token_2022::{
            extension::{confidential_transfer, confidential_transfer_fee, ExtensionType},
            state::Mint,
        },
        InitializeMint2, InitializeMintCloseAuthority, Token2022,
    },
    token_interface::{
        default_account_state_initialize, metadata_pointer_initialize, transfer_fee_initialize,
        DefaultAccountStateInitialize, MetadataPointerInitialize, TransferFeeInitialize,
    },
};

use crate::{constants::*, state::Config};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = Config::SIZE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,

    /// CHECK: use unchecked account becasuse some extensions
    #[account(mut, signer)]
    pub mint: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> Initialize<'info> {
    pub fn process_ix(
        &mut self,
        transfer_fee_bps: u16,
        maximum_fee: u64,
        decimals: u8,
        bumps: &InitializeBumps,
        elgamal_pubkey: [u8; 32],
    ) -> Result<()> {
        // initialize config account
        self.config.set_inner(Config {
            authority: self.authority.key(),
            mint: self.mint.key(),
            bump: bumps.config,
        });

        let extensions = [
            ExtensionType::TransferFeeConfig,
            ExtensionType::MetadataPointer,
            ExtensionType::DefaultAccountState,
            ExtensionType::MintCloseAuthority,
            // ExtensionType::PermanentDelegate,
            ExtensionType::ConfidentialTransferMint,
            ExtensionType::ConfidentialTransferFeeConfig,
        ];

        // Phase 1: allocate at the full extended length. Getting this number
        // from anywhere other than `try_calculate_account_len` is how mints
        // end up too small to initialize.
        let space = ExtensionType::try_calculate_account_len::<Mint>(&extensions)?;
        let lamports = Rent::get()?.minimum_balance(space);

        system_program::create_account(
            CpiContext::new(
                self.system_program.key(),
                anchor_lang::system_program::CreateAccount {
                    from: self.authority.to_account_info(),
                    to: self.mint.to_account_info(),
                },
            ),
            lamports,
            space as u64,
            &self.token_program.key(),
        )?;

        // Phase 2: initialize each extension, before the mint itself exists.
        transfer_fee_initialize(
            CpiContext::new(
                self.token_program.key(),
                TransferFeeInitialize {
                    token_program_id: self.token_program.to_account_info(),
                    mint: self.mint.to_account_info(),
                },
            ),
            Some(&self.authority.key()),
            Some(&self.authority.key()),
            transfer_fee_bps,
            maximum_fee,
        )?;

        metadata_pointer_initialize(
            CpiContext::new(
                self.token_program.key(),
                MetadataPointerInitialize {
                    token_program_id: self.token_program.to_account_info(),
                    mint: self.mint.to_account_info(),
                },
            ),
            Some(self.authority.key()),
            Some(self.mint.key()), // pointing to itself
        )?;

        default_account_state_initialize(
            CpiContext::new(
                self.token_program.key(),
                DefaultAccountStateInitialize {
                    token_program_id: self.token_program.to_account_info(),
                    mint: self.mint.to_account_info(),
                },
            ),
            &anchor_spl::token_2022::spl_token_2022::state::AccountState::Frozen, // until kyc
        )?;

        initialize_mint_close_authority(
            CpiContext::new(
                self.token_program.key(),
                InitializeMintCloseAuthority {
                    mint: self.mint.to_account_info(),
                },
            ),
            Some(&self.authority.key()),
        )?;

        let ix = confidential_transfer::instruction::initialize_mint(
            &self.token_program.key(),
            &self.mint.key(),
            Some(self.authority.key()),
            false,
            None,
        )?;
        invoke(
            &ix,
            &[
                self.mint.to_account_info(),
                self.token_program.to_account_info(),
            ],
        )?;
        let ix =
            confidential_transfer_fee::instruction::initialize_confidential_transfer_fee_config(
                &self.token_program.key(),
                &self.mint.key(),
                Some(self.authority.key()),
                &elgamal_pubkey.into(),
            )?;
        invoke(
            &ix,
            &[
                self.mint.to_account_info(),
                self.token_program.to_account_info(),
            ],
        )?;

        // Phase 3: seal the mint. Nothing can be added after this point, and
        // most mint extensions cannot be added later at all, so a mistake here
        // is permanent rather than recoverable.
        initialize_mint2(
            CpiContext::new(
                self.token_program.key(),
                InitializeMint2 {
                    mint: self.mint.to_account_info(),
                },
            ),
            decimals,
            &self.authority.key(),
            Some(&self.authority.key()),
        )?;

        Ok(())
    }
}
