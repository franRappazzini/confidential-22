use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction::create_account;
use spl_associated_token_account_interface::{
    address::get_associated_token_address_with_program_id,
    instruction::create_associated_token_account,
};
use spl_token_2022_interface::{
    extension::{
        default_account_state::{
            instruction::{initialize_default_account_state, update_default_account_state},
            DefaultAccountState,
        },
        BaseStateWithExtensions, ExtensionType, StateWithExtensions,
    },
    instruction::initialize_mint,
    state::{Account, AccountState, Mint},
    ID as TOKEN_2022_PROGRAM_ID,
};

fn get_account_state_label(state: AccountState) -> &'static str {
    match state {
        AccountState::Frozen => "Frozen",
        AccountState::Initialized => "Initialized",
        AccountState::Uninitialized => "Uninitialized",
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let client = RpcClient::new_with_commitment(
        String::from("http://localhost:8899"),
        CommitmentConfig::confirmed(),
    );
    let fee_payer = Keypair::new();

    let airdrop_signature = client
        .request_airdrop(&fee_payer.pubkey(), 5_000_000_000)
        .await?;

    loop {
        let confirmed = client.confirm_transaction(&airdrop_signature).await?;
        if confirmed {
            break;
        }
    }

    let mint = Keypair::new();

    let mint_space =
        ExtensionType::try_calculate_account_len::<Mint>(&[ExtensionType::DefaultAccountState])?;

    let mint_rent = client
        .get_minimum_balance_for_rent_exemption(mint_space)
        .await?;

    let create_mint_account_instruction = create_account(
        &fee_payer.pubkey(),    // Account funding the new mint account.
        &mint.pubkey(),         // New mint account to create.
        mint_rent,              // Lamports funding the mint account rent.
        mint_space as u64,      // Account size in bytes for the mint plus DefaultAccountState.
        &TOKEN_2022_PROGRAM_ID, // Program that owns the mint account.
    );

    let initialize_default_account_state_instruction = initialize_default_account_state(
        &TOKEN_2022_PROGRAM_ID, // Token program that owns the mint.
        &mint.pubkey(), // Mint account that stores the DefaultAccountState extension.
        &AccountState::Frozen, // Default state assigned to new token accounts.
    )?;

    let initialize_mint_instruction = initialize_mint(
        &TOKEN_2022_PROGRAM_ID,    // Program that owns the mint account.
        &mint.pubkey(),            // Mint account to initialize.
        &fee_payer.pubkey(),       // Authority allowed to mint new tokens.
        Some(&fee_payer.pubkey()), // Authority allowed to freeze token accounts.
        0,                         // Number of decimals for the token.
    )?;

    let create_mint_transaction = Transaction::new_signed_with_payer(
        &[
            create_mint_account_instruction,
            initialize_default_account_state_instruction,
            initialize_mint_instruction,
        ],
        Some(&fee_payer.pubkey()),
        &[&fee_payer, &mint],
        client.get_latest_blockhash().await?,
    );

    client
        .send_and_confirm_transaction(&create_mint_transaction)
        .await?;

    let token_account = get_associated_token_address_with_program_id(
        &fee_payer.pubkey(),
        &mint.pubkey(),
        &TOKEN_2022_PROGRAM_ID,
    );

    let create_token_account_transaction = Transaction::new_signed_with_payer(
        &[create_associated_token_account(
            &fee_payer.pubkey(), // Account funding the associated token account creation.
            &fee_payer.pubkey(), // Owner of the token account.
            &mint.pubkey(), // Mint for the associated token account.
            &TOKEN_2022_PROGRAM_ID, // Token program that owns the token account.
        )],
        Some(&fee_payer.pubkey()),
        &[&fee_payer],
        client.get_latest_blockhash().await?,
    );

    client
        .send_and_confirm_transaction(&create_token_account_transaction)
        .await?;

    let token_account_data_before_update = client.get_account(&token_account).await?;
    let token_account_state_before_update =
        StateWithExtensions::<Account>::unpack(&token_account_data_before_update.data)?;
    let mint_account_before_update = client.get_account(&mint.pubkey()).await?;
    let mint_state_before_update =
        StateWithExtensions::<Mint>::unpack(&mint_account_before_update.data)?;
    let default_account_state_before_update = mint_state_before_update
        .get_extension::<DefaultAccountState>()?;

    let update_default_account_state_instruction = update_default_account_state(
        &TOKEN_2022_PROGRAM_ID, // Token program that owns the mint.
        &mint.pubkey(), // Mint account that stores the DefaultAccountState extension.
        &fee_payer.pubkey(), // Freeze authority authorized to update the default state.
        &[&fee_payer.pubkey()], // Additional multisig signers.
        &AccountState::Initialized, // New default state assigned to later token accounts.
    )?;

    let update_default_account_state_transaction = Transaction::new_signed_with_payer(
        &[update_default_account_state_instruction],
        Some(&fee_payer.pubkey()),
        &[&fee_payer],
        client.get_latest_blockhash().await?,
    );

    client
        .send_and_confirm_transaction(&update_default_account_state_transaction)
        .await?;

    let token_account_data_after_update = client.get_account(&token_account).await?;
    let token_account_state_after_update =
        StateWithExtensions::<Account>::unpack(&token_account_data_after_update.data)?;
    let mint_account_after_update = client.get_account(&mint.pubkey()).await?;
    let mint_state_after_update =
        StateWithExtensions::<Mint>::unpack(&mint_account_after_update.data)?;
    let default_account_state_after_update =
        mint_state_after_update.get_extension::<DefaultAccountState>()?;

    println!("Mint Address: {}", mint.pubkey());
    println!(
        "Default Account State Before Update: {:?}",
        default_account_state_before_update
    );
    println!(
        "Token Account State Before Update: {}",
        get_account_state_label(token_account_state_before_update.base.state)
    );
    println!(
        "Default Account State After Update: {:?}",
        default_account_state_after_update
    );
    println!(
        "Token Account State After Update: {}",
        get_account_state_label(token_account_state_after_update.base.state)
    );

    Ok(())
}