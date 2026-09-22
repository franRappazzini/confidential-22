mod ixs;
mod utils;

use proofext::instruction::ProofLocation;
use solana_keypair::Keypair;
use solana_signer::Signer;
use t22new::{
    extension::{
        confidential_transfer, confidential_transfer_fee::ConfidentialTransferFeeAmount,
        default_account_state::DefaultAccountState, BaseStateWithExtensions, StateWithExtensions,
    },
    state::{Account, Mint},
};
use zk::encryption::{
    auth_encryption::AeCiphertext,
    derivation::derive_confidential_keys,
    elgamal::{ElGamalCiphertext, ElGamalPubkey},
};
use zkif::instruction::ProofInstruction;

#[test]
fn initialize() {
    let (mut svm, authority, ..) = utils::setup();
    let config = utils::pdas();

    let (fee_authority_elgamal, _) = derive_confidential_keys(&authority, b"").unwrap();
    let fee_authority_pubkey: [u8; 32] = fee_authority_elgamal.pubkey().into();

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
        fee_authority_pubkey,
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

    let (fee_authority_elgamal, _) = derive_confidential_keys(&authority, b"").unwrap();
    let fee_authority_pubkey: [u8; 32] = fee_authority_elgamal.pubkey().into();

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
        fee_authority_pubkey,
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
    let (mut svm, authority, _user, user2) = utils::setup();
    let config = utils::pdas();
    let mint_keypair = Keypair::new();

    let (fee_authority_elgamal, _) = derive_confidential_keys(&authority, b"").unwrap();
    let fee_authority_pubkey: [u8; 32] = fee_authority_elgamal.pubkey().into();

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
        fee_authority_pubkey,
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

    let user = authority.insecure_clone();
    let user_ata = utils::token(&mut svm, &user, mint_keypair.pubkey());

    let ix2 = ixs::create_unfreeze_ix(&authority, config, &mint_keypair, user_ata);

    utils::send_tx(&mut svm, &[ix2], &authority, &[&authority], false);

    // ---- confidential ----
    let holder1 =
        ixs::transfer_handler::configure_fee_holder(&mut svm, &mint_keypair.pubkey(), &user);
    let holder2 =
        ixs::transfer_handler::configure_fee_holder(&mut svm, &mint_keypair.pubkey(), &user2);

    // first unfreeze atas
    let ix1 = ixs::create_unfreeze_ix(&authority, config, &mint_keypair, holder1.account);
    let ix2 = ixs::create_unfreeze_ix(&authority, config, &mint_keypair, holder2.account);

    utils::send_tx(&mut svm, &[ix1, ix2], &authority, &[&authority], false);

    // TODO hacer aprobacion manual ?

    // Fund holder1 and move it into her confidential available balance.
    ixs::transfer_handler::mint_to_confidential(&mut svm, &mint_keypair, &user, &holder1);
    println!("==== pasa ====");

    ixs::transfer_handler::apply_pending(&mut svm, &authority, &holder1, &user);

    println!("==== pasa ====");

    let holder1_available = ixs::transfer_handler::available_balance(
        &ixs::transfer_handler::read_ct(&svm, &holder1.account),
        &holder1.elgamal,
    );
    assert_eq!(holder1_available, 100_000);
    assert_eq!(
        ixs::transfer_handler::withheld_on_account(&svm, &holder2.account, &fee_authority_elgamal),
        0
    );

    // ---- the fee bearing transfer ----------------------------------------
    let transfer_amount = 10_000u64;
    let ct = ixs::transfer_handler::read_ct(&svm, &holder1.account);
    let current_available: ElGamalCiphertext = ct.available_balance.try_into().unwrap();
    let current_decryptable: AeCiphertext = ct.decryptable_available_balance.try_into().unwrap();
    let holder2_pubkey: ElGamalPubkey = ixs::transfer_handler::read_ct(&svm, &holder2.account)
        .elgamal_pubkey
        .try_into()
        .unwrap();

    // Five proofs now. The two extra ones exist because the fee is
    // a percentage of an amount nobody can see:
    //
    //   percentage_with_cap  proves the fee was computed correctly from the
    //                        hidden transfer amount, at the mint's rate and
    //                        capped at the mint's maximum
    //   fee_ciphertext_validity
    //                        proves the withheld fee ciphertext is well formed
    //                        under both the destination and the fee authority
    //                        keys
    //
    // The range proof also widens from U128 to U256, because there are more
    // committed values to bound.
    let proofs = proofgen::transfer_with_fee::transfer_with_fee_split_proof_data(
        &current_available,
        &current_decryptable,
        transfer_amount,
        &holder1.elgamal,
        &holder1.aes,
        &holder2_pubkey,
        None,
        fee_authority_elgamal.pubkey(),
        transfer_fee_bps,
        maximum_fee,
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
            .transfer_amount_ciphertext_validity_proof_data_with_ciphertext
            .proof_data,
    );
    let pct_ctx = ixs::transfer_handler::stage_proof(
        &mut svm,
        &authority,
        ProofInstruction::VerifyPercentageWithCap,
        &proofs.percentage_with_cap_proof_data,
    );
    let fee_val_ctx = ixs::transfer_handler::stage_proof(
        &mut svm,
        &authority,
        ProofInstruction::VerifyBatchedGroupedCiphertext2HandlesValidity,
        &proofs.fee_ciphertext_validity_proof_data,
    );
    let range_ctx = ixs::transfer_handler::stage_proof(
        &mut svm,
        &authority,
        ProofInstruction::VerifyBatchedRangeProofU256,
        &proofs.range_proof_data,
    );

    let new_holder1_decryptable = holder1.aes.encrypt(holder1_available - transfer_amount);
    let ixs = confidential_transfer::instruction::transfer_with_fee(
        &t22new::ID,
        &holder1.account,
        &mint_keypair.pubkey(),
        &holder2.account,
        &new_holder1_decryptable.into(),
        &proofs
            .transfer_amount_ciphertext_validity_proof_data_with_ciphertext
            .ciphertext_lo,
        &proofs
            .transfer_amount_ciphertext_validity_proof_data_with_ciphertext
            .ciphertext_hi,
        &user.pubkey(),
        &[],
        ProofLocation::ContextStateAccount(&eq_ctx),
        ProofLocation::ContextStateAccount(&val_ctx),
        ProofLocation::ContextStateAccount(&pct_ctx),
        ProofLocation::ContextStateAccount(&fee_val_ctx),
        ProofLocation::ContextStateAccount(&range_ctx),
    )
    .unwrap();

    utils::send_tx(&mut svm, &ixs, &authority, &[&user, &authority], false);

    ixs::transfer_handler::close_contexts(
        &mut svm,
        &authority,
        &[eq_ctx, val_ctx, pct_ctx, fee_val_ctx, range_ctx],
    );

    // ---- what the fee did -------------------------------------------------
    let expected_fee = transfer_amount * u64::from(transfer_fee_bps) / 10_000;

    // The fee is withheld on the recipient's account, not deducted from the
    // sender. holder1 is debited the full amount.
    let holder1_after = ixs::transfer_handler::available_balance(
        &ixs::transfer_handler::read_ct(&svm, &holder1.account),
        &holder1.elgamal,
    );
    assert_eq!(holder1_after, holder1_available - transfer_amount);

    // holder2 receives the amount minus the fee, still in pending.
    let holder2_pending = ixs::transfer_handler::pending_balance(
        &ixs::transfer_handler::read_ct(&svm, &holder2.account),
        &holder2.elgamal,
    );
    println!("transfer={transfer_amount} fee={expected_fee} holder2_pending={holder2_pending}");
    assert_eq!(holder2_pending, transfer_amount - expected_fee);

    // And the fee sits on holder2's account, readable only by the fee authority.
    let withheld =
        ixs::transfer_handler::withheld_on_account(&svm, &holder2.account, &fee_authority_elgamal);
    println!("withheld on holder2's account = {withheld}");
    assert_eq!(withheld, expected_fee);

    // holder2 cannot read it. The ciphertext is under the fee authority's key.
    let acct = svm.get_account(&holder2.account).unwrap();
    let state = StateWithExtensions::<Account>::unpack(&acct.data).unwrap();
    let raw: ElGamalCiphertext = state
        .get_extension::<ConfidentialTransferFeeAmount>()
        .unwrap()
        .withheld_amount
        .try_into()
        .unwrap();
    assert_ne!(
        holder2.elgamal.secret().decrypt_u32(&raw),
        Some(expected_fee)
    );
}
