mod ixs;
mod utils;

use anchor_spl::token_2022::spl_token_2022::{
    extension::{
        default_account_state::DefaultAccountState, BaseStateWithExtensions, StateWithExtensions,
    },
    state::{Account, Mint},
};

use proofext::instruction::ProofLocation;
use solana_keypair::Keypair;
use solana_signer::Signer;
use t22new::extension::confidential_transfer;
use zk::encryption::{
    auth_encryption::AeCiphertext,
    elgamal::{ElGamalCiphertext, ElGamalPubkey},
};
use zkif::instruction::ProofInstruction;

#[test]
fn initialize() {
    let (mut svm, authority, ..) = utils::setup();
    let config = utils::pdas();

    let transfer_fee_bps = 500;
    let maximum_fee = 1_000;
    let decimals = 6;

    let mint_keypair = Keypair::new();

    let ix = ixs::create_initialixe_ix(
        &authority,
        config,
        &mint_keypair,
        decimals,
        transfer_fee_bps,
        maximum_fee,
    );

    utils::send_tx(
        &mut svm,
        &[ix],
        &authority,
        &[&authority, &mint_keypair],
        true,
    );
}

#[test]
fn unfreeze() {
    let (mut svm, authority, user, ..) = utils::setup();
    let config = utils::pdas();
    let mint_keypair = Keypair::new();

    let transfer_fee_bps = 500;
    let maximum_fee = 1_000;
    let decimals = 6;

    let ix1 = ixs::create_initialixe_ix(
        &authority,
        config,
        &mint_keypair,
        decimals,
        transfer_fee_bps,
        maximum_fee,
    );
    // fist send the initialize ix to create the token mint
    utils::send_tx(
        &mut svm,
        &[ix1],
        &authority,
        &[&authority, &mint_keypair],
        false,
    );

    let mint_account_before_update = svm.get_account(&mint_keypair.pubkey()).unwrap();
    let mint_state_before_update =
        StateWithExtensions::<Mint>::unpack(&mint_account_before_update.data).unwrap();

    let default_account_state_before_update = mint_state_before_update
        .get_extension::<DefaultAccountState>()
        .unwrap();

    println!("{:#?}", default_account_state_before_update);

    let user_ata = utils::token(&mut svm, &user, mint_keypair.pubkey());

    let ix2 = ixs::create_unfreeze_ix(&authority, config, &mint_keypair, user_ata);

    utils::send_tx(&mut svm, &[ix2], &authority, &[&authority], true);
}

#[test]
fn confidential_flow() {
    let (mut svm, authority, user, user2) = utils::setup();
    let config = utils::pdas();
    let mint_keypair = Keypair::new();

    let transfer_fee_bps = 500;
    let maximum_fee = 1_000;
    let decimals = 6;

    let ix1 = ixs::create_initialixe_ix(
        &authority,
        config,
        &mint_keypair,
        decimals,
        transfer_fee_bps,
        maximum_fee,
    );
    // fist send the initialize ix to create the token mint
    utils::send_tx(
        &mut svm,
        &[ix1],
        &authority,
        &[&authority, &mint_keypair],
        false,
    );

    let mint_account_before_update = svm.get_account(&mint_keypair.pubkey()).unwrap();
    let mint_state_before_update =
        StateWithExtensions::<Mint>::unpack(&mint_account_before_update.data).unwrap();

    let default_account_state_before_update = mint_state_before_update
        .get_extension::<DefaultAccountState>()
        .unwrap();

    println!("{:#?}", default_account_state_before_update);

    let user_ata = utils::token(&mut svm, &user, mint_keypair.pubkey());

    let ix2 = ixs::create_unfreeze_ix(&authority, config, &mint_keypair, user_ata);

    utils::send_tx(&mut svm, &[ix2], &authority, &[&authority], false);

    // ---- confidential ----

    let holder1 = ixs::transfer_handler::create_and_configure(
        &mut svm,
        &authority,
        &mint_keypair.pubkey(),
        &user,
    );
    let holder2 = ixs::transfer_handler::create_and_configure(
        &mut svm,
        &authority,
        &mint_keypair.pubkey(),
        &user2,
    );

    // mint to
    ixs::transfer_handler::mint_to_confidential(&mut svm, &mint_keypair, &authority, &holder1);
    ixs::transfer_handler::mint_to_confidential(&mut svm, &mint_keypair, &authority, &holder2);

    let ct = ixs::transfer_handler::read_ct(&svm, &holder1.account);
    println!(
        "after deposit: pending={} available={}",
        ixs::transfer_handler::pending_balance(&ct, &holder1.elgamal),
        ixs::transfer_handler::available_balance(&ct, &holder1.elgamal)
    );

    //  apply pending balance
    ixs::transfer_handler::apply_pending(&mut svm, &authority, &holder1, &user);
    let ct = ixs::transfer_handler::read_ct(&svm, &holder1.account);
    let holder1_available = ixs::transfer_handler::available_balance(&ct, &holder1.elgamal);
    println!(
        "after apply: pending={} available={}",
        ixs::transfer_handler::pending_balance(&ct, &holder1.elgamal),
        holder1_available
    );
    assert_eq!(holder1_available, 10_000);

    // ---- confidential transfer to holder2 -------------------------------------
    let transfer_amount = 2_500u64;
    let ct = ixs::transfer_handler::read_ct(&svm, &holder1.account);
    let current_available: ElGamalCiphertext = ct.available_balance.try_into().unwrap();
    let current_decryptable: AeCiphertext = ct.decryptable_available_balance.try_into().unwrap();
    let holder2_ct = ixs::transfer_handler::read_ct(&svm, &holder2.account);
    let holder2_pubkey: ElGamalPubkey = holder2_ct.elgamal_pubkey.try_into().unwrap();

    let proofs = proofgen::transfer::transfer_split_proof_data(
        &current_available,
        &current_decryptable,
        transfer_amount,
        &holder1.elgamal,
        &holder1.aes,
        &holder2_pubkey,
        None,
    )
    .unwrap();

    let eq_ctx = ixs::transfer_handler::stage_proof(
        &mut svm,
        &authority,
        ProofInstruction::VerifyCiphertextCommitmentEquality,
        &proofs.equality_proof_data,
    );
    let val_ctx = ixs::transfer_handler::stage_proof(
        &mut svm,
        &authority,
        ProofInstruction::VerifyBatchedGroupedCiphertext3HandlesValidity,
        &proofs
            .ciphertext_validity_proof_data_with_ciphertext
            .proof_data,
    );
    let range_ctx = ixs::transfer_handler::stage_proof(
        &mut svm,
        &authority,
        ProofInstruction::VerifyBatchedRangeProofU128,
        &proofs.range_proof_data,
    );

    let holder1_decryptable = holder1.aes.encrypt(holder1_available - transfer_amount);
    let ixs = confidential_transfer::instruction::transfer(
        &t22new::ID,
        &holder1.account,
        &mint_keypair.pubkey(),
        &holder2.account,
        &holder1_decryptable.into(),
        &proofs
            .ciphertext_validity_proof_data_with_ciphertext
            .ciphertext_lo,
        &proofs
            .ciphertext_validity_proof_data_with_ciphertext
            .ciphertext_hi,
        &user.pubkey(),
        &[],
        ProofLocation::ContextStateAccount(&eq_ctx),
        ProofLocation::ContextStateAccount(&val_ctx),
        ProofLocation::ContextStateAccount(&range_ctx),
    )
    .unwrap();
    utils::send_tx(&mut svm, &ixs, &authority, &[&user, &authority], false);

    let recovered =
        ixs::transfer_handler::close_contexts(&mut svm, &authority, &[eq_ctx, val_ctx, range_ctx]);
    println!("rent recovered from transfer proofs = {recovered} lamports");

    // holder2's incoming amount lands in pending, not available.
    let holder2_ct = ixs::transfer_handler::read_ct(&svm, &holder2.account);
    println!(
        "holder2 after transfer: pending={} available={}",
        ixs::transfer_handler::pending_balance(&holder2_ct, &holder2.elgamal),
        ixs::transfer_handler::available_balance(&holder2_ct, &holder2.elgamal)
    );
    assert_eq!(
        ixs::transfer_handler::pending_balance(&holder2_ct, &holder2.elgamal),
        transfer_amount
    );
    assert_eq!(
        ixs::transfer_handler::available_balance(&holder2_ct, &holder2.elgamal),
        0
    );

    ixs::transfer_handler::apply_pending(&mut svm, &authority, &holder2, &user2);
    let holder2_ct = ixs::transfer_handler::read_ct(&svm, &holder2.account);
    assert_eq!(
        ixs::transfer_handler::available_balance(&holder2_ct, &holder2.elgamal),
        transfer_amount
    );
    println!("holder2 after apply: available={}", transfer_amount);

    // ---- withdraw back to the public balance ------------------------------
    let withdraw_amount = 1_000u64;
    let holder2_ct = ixs::transfer_handler::read_ct(&svm, &holder2.account);
    let holder2_available = ixs::transfer_handler::available_balance(&holder2_ct, &holder2.elgamal);
    let holder2_current: ElGamalCiphertext = holder2_ct.available_balance.try_into().unwrap();

    let wproofs = proofgen::withdraw::withdraw_proof_data(
        &holder2_current,
        holder2_available,
        withdraw_amount,
        &holder2.elgamal,
    )
    .unwrap();

    let weq_ctx = ixs::transfer_handler::stage_proof(
        &mut svm,
        &authority,
        ProofInstruction::VerifyCiphertextCommitmentEquality,
        &wproofs.equality_proof_data,
    );
    let wrange_ctx = ixs::transfer_handler::stage_proof(
        &mut svm,
        &authority,
        ProofInstruction::VerifyBatchedRangeProofU64,
        &wproofs.range_proof_data,
    );

    let new_holder2_decryptable = holder2.aes.encrypt(holder2_available - withdraw_amount);
    let ixs = confidential_transfer::instruction::withdraw(
        &t22new::ID,
        &holder2.account,
        &mint_keypair.pubkey(),
        withdraw_amount,
        6,
        &new_holder2_decryptable.into(),
        &user2.pubkey(),
        &[],
        ProofLocation::ContextStateAccount(&weq_ctx),
        ProofLocation::ContextStateAccount(&wrange_ctx),
    )
    .unwrap();
    utils::send_tx(&mut svm, &ixs, &authority, &[&user2, &authority], false);

    let recovered =
        ixs::transfer_handler::close_contexts(&mut svm, &authority, &[weq_ctx, wrange_ctx]);
    println!("rent recovered from withdraw proofs = {recovered} lamports");

    let acct = svm.get_account(&holder2.account).unwrap();
    let state = StateWithExtensions::<Account>::unpack(&acct.data).unwrap();
    println!("holder2 public balance = {}", state.base.amount);
    assert_eq!(state.base.amount, withdraw_amount);

    let holder2_ct = ixs::transfer_handler::read_ct(&svm, &holder2.account);
    assert_eq!(
        ixs::transfer_handler::available_balance(&holder2_ct, &holder2.elgamal),
        transfer_amount - withdraw_amount
    );
    println!(
        "holder2 confidential available = {}",
        transfer_amount - withdraw_amount
    );
}
