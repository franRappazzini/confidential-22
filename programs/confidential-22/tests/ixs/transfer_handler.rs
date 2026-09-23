use crate::utils;

use anchor_lang::{
    prelude::Pubkey,
    solana_program::{instruction::Instruction, system_program},
    InstructionData, ToAccountMetas,
};
use litesvm::LiteSVM;
use proofext::instruction::ProofLocation;
use proofgen::{
    transfer::transfer_split_proof_data, transfer_with_fee::transfer_with_fee_split_proof_data,
    withdraw::withdraw_proof_data,
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::num::NonZeroI8;
use t22new::{
    extension::{
        confidential_transfer::{instruction as ct_ix, ConfidentialTransferAccount},
        confidential_transfer_fee::{
            instruction::{disable_harvest_to_mint, enable_harvest_to_mint},
            ConfidentialTransferFeeAmount, ConfidentialTransferFeeConfig,
        },
        BaseStateWithExtensions, ExtensionType, StateWithExtensions,
    },
    instruction::{initialize_account3, mint_to},
    state::{Account as TokenAccountState, Mint as MintState},
};
use zk::{
    encryption::{
        auth_encryption::{AeCiphertext, AeKey},
        derivation::derive_confidential_keys,
        elgamal::{ElGamalCiphertext, ElGamalKeypair, ElGamalPubkey},
    },
    zk_elgamal_proof_program::pubkey_validity::build_pubkey_validity_proof_data,
};
use zkif::{
    instruction::{close_context_state, ContextStateInfo, ProofInstruction},
    proof_data::ZkProofData,
    state::ProofContextState,
};

pub struct Holder {
    pub account: Pubkey,
    pub elgamal: ElGamalKeypair,
    pub aes: AeKey,
}

pub fn create_and_configure(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    owner: &Keypair,
) -> Holder {
    let ctoken_account = Keypair::new();
    let space = ExtensionType::try_calculate_account_len::<TokenAccountState>(&[
        ExtensionType::ConfidentialTransferAccount,
    ])
    .unwrap();
    let lamports = svm.minimum_balance_for_rent_exemption(space);

    utils::send_tx(
        svm,
        &[
            solana_system_interface::instruction::create_account(
                &payer.pubkey(),
                &ctoken_account.pubkey(),
                lamports,
                space as u64,
                &t22new::ID,
            ),
            initialize_account3(&t22new::ID, &ctoken_account.pubkey(), mint, &owner.pubkey())
                .unwrap(),
        ],
        payer,
        &[&ctoken_account, &payer],
        false,
    );

    // Both keys come from one signature over a fixed derivation message, via
    // HKDF-SHA512. Three things worth saying out loud:
    //
    //   - The owner can always recover them from their wallet. Nothing is
    //     stored, and losing them means losing the ability to read the balance.
    //   - Whatever can produce that signature can decrypt every confidential
    //     balance the wallet holds.
    //   - The empty seed is the standard, wallet level derivation. It is
    //     byte identical to what the spl-token CLI and the JavaScript client
    //     derive, so those tools can read accounts configured here. A non
    //     empty seed scopes keys more finely but breaks that interoperability.
    let (elgamal, aes) = derive_confidential_keys(owner, b"").unwrap();
    let proof = build_pubkey_validity_proof_data(&elgamal).unwrap();

    let ixs = ct_ix::configure_account(
        &t22new::ID,
        &ctoken_account.pubkey(),
        mint,
        &aes.encrypt(0).into(),
        65536,
        &owner.pubkey(),
        &[],
        ProofLocation::InstructionOffset(NonZeroI8::new(1).unwrap(), &proof),
    )
    .unwrap();

    utils::send_tx(svm, &ixs, payer, &[owner, payer], false);
    println!("--- pasa ---");

    Holder {
        account: ctoken_account.pubkey(),
        elgamal,
        aes,
    }
}

/// Create a fee bearing confidential holder: a token account sized for all
/// three account extensions, initialized, and configured.
pub fn configure_fee_holder(svm: &mut LiteSVM, mint: &Pubkey, owner: &Keypair) -> Holder {
    let space = ExtensionType::try_calculate_account_len::<TokenAccountState>(&[
        ExtensionType::TransferFeeAmount,
        ExtensionType::ConfidentialTransferAccount,
        ExtensionType::ConfidentialTransferFeeAmount,
    ])
    .unwrap();
    let ta = Keypair::new();
    let lamports = svm.minimum_balance_for_rent_exemption(space);
    utils::send_tx(
        svm,
        &[
            solana_system_interface::instruction::create_account(
                &owner.pubkey(),
                &ta.pubkey(),
                lamports,
                space as u64,
                &t22new::ID,
            ),
            initialize_account3(&t22new::ID, &ta.pubkey(), mint, &owner.pubkey()).unwrap(),
        ],
        owner,
        &[&ta, owner],
        false,
    );

    let (elgamal, aes) = derive_confidential_keys(owner, b"").unwrap();
    let proof = build_pubkey_validity_proof_data(&elgamal).unwrap();
    let ixs = ct_ix::configure_account(
        &t22new::ID,
        &ta.pubkey(),
        mint,
        &aes.encrypt(0).into(),
        65536,
        &owner.pubkey(),
        &[],
        ProofLocation::InstructionOffset(NonZeroI8::new(1).unwrap(), &proof),
    )
    .unwrap();
    utils::send_tx(svm, &ixs, owner, &[owner], false);

    Holder {
        account: ta.pubkey(),
        elgamal,
        aes,
    }
}

/// Read the withheld fee sitting on a token account. Encrypted under the fee
/// authority's key, so only that key can read it, not the holder's.
pub fn withheld_on_account(svm: &LiteSVM, account: &Pubkey, fee_authority: &ElGamalKeypair) -> u64 {
    let acct = svm.get_account(account).unwrap();
    let state = StateWithExtensions::<TokenAccountState>::unpack(&acct.data).unwrap();
    let ext = state
        .get_extension::<ConfidentialTransferFeeAmount>()
        .unwrap();
    let ciphertext: ElGamalCiphertext = ext.withheld_amount.try_into().unwrap();
    fee_authority.secret().decrypt_u32(&ciphertext).unwrap()
}

pub fn mint_to_confidential(
    svm: &mut LiteSVM,
    mint: &Keypair,
    authority: &Keypair,
    holder: &Holder,
    amount: u64,
) {
    utils::send_tx(
        svm,
        &[mint_to(
            &t22new::ID,
            &mint.pubkey(),
            &holder.account,
            &authority.pubkey(),
            &[],
            amount,
        )
        .unwrap()],
        &authority,
        &[authority],
        false,
    );

    let config = utils::pdas();

    utils::send_tx(
        svm,
        &[Instruction {
            program_id: confidential_22::ID,
            accounts: confidential_22::accounts::DepositConfidential {
                authority: authority.pubkey(),
                config,
                mint: mint.pubkey(),
                ata: holder.account,
                token_program: t22new::ID,
            }
            .to_account_metas(None),
            data: confidential_22::instruction::DepositConfidential { amount }.data(),
        }],
        &authority,
        &[authority],
        false,
    );
}

/// Read the confidential extension off a token account.
pub fn read_ct(svm: &LiteSVM, account: &Pubkey) -> ConfidentialTransferAccount {
    let acct = svm.get_account(account).unwrap();
    let state = StateWithExtensions::<TokenAccountState>::unpack(&acct.data).unwrap();
    *state
        .get_extension::<ConfidentialTransferAccount>()
        .unwrap()
}

/// Decrypt the available balance. Only the owner can do this, which is the
/// whole point of the extension.
pub fn available_balance(ct: &ConfidentialTransferAccount, elgamal: &ElGamalKeypair) -> u64 {
    let ciphertext: ElGamalCiphertext = ct.available_balance.try_into().unwrap();
    elgamal.secret().decrypt_u32(&ciphertext).unwrap()
}

/// Decrypt the pending balance, which is stored as a low and a high component
/// because ElGamal decryption is a discrete log search and has to stay cheap.
pub fn pending_balance(ct: &ConfidentialTransferAccount, elgamal: &ElGamalKeypair) -> u64 {
    let lo: ElGamalCiphertext = ct.pending_balance_lo.try_into().unwrap();
    let hi: ElGamalCiphertext = ct.pending_balance_hi.try_into().unwrap();
    let lo = elgamal.secret().decrypt_u32(&lo).unwrap();
    let hi = elgamal.secret().decrypt_u32(&hi).unwrap();
    lo + (hi << 16)
}

/// Move pending into available.
///
/// The program cannot compute the new decryptable balance, because it has no
/// access to the owner's AES key. The client decrypts, adds, re-encrypts, and
/// hands the ciphertext in as an instruction argument.
pub fn apply_pending(svm: &mut LiteSVM, authority: &Keypair, holder: &Holder, owner: &Keypair) {
    let ct = read_ct(svm, &holder.account);
    let counter: u64 = ct.pending_balance_credit_counter.into();
    let new_available =
        available_balance(&ct, &holder.elgamal) + pending_balance(&ct, &holder.elgamal);
    let ciphertext: [u8; 36] = holder.aes.encrypt(new_available).to_bytes();

    let ix = Instruction {
        program_id: confidential_22::ID,
        accounts: confidential_22::accounts::ApplyPendingBalance {
            ata: holder.account,
            authority: owner.pubkey(),
            token_program: t22new::ID,
        }
        .to_account_metas(None),
        data: confidential_22::instruction::ApplyPendingBalance {
            expected_pending_balance_credit_counter: counter,
            new_decryptable_available_balance: ciphertext,
        }
        .data(),
    };

    utils::send_tx(svm, &[ix], authority, &[owner], false);
}

/// Verify a proof into its own context state account, so the token instruction
/// can reference it instead of carrying it inline. A confidential transfer's
/// three proofs do not fit in one 1232 byte transaction, which is why the
/// lifecycle needs several dependent transactions.
pub fn stage_proof<T, U>(
    svm: &mut LiteSVM,
    payer: &Keypair,
    instruction_kind: ProofInstruction,
    proof: &T,
) -> Pubkey
where
    T: bytemuck::Pod + ZkProofData<U>,
    U: bytemuck::Pod,
{
    // Size comes from the proof's own context type rather than a magic
    // number: authority, proof type tag, then the context data itself.
    let context_len = std::mem::size_of::<ProofContextState<U>>();
    let context = Keypair::new();
    let lamports = svm.minimum_balance_for_rent_exemption(context_len);
    utils::send_tx(
        svm,
        &[solana_system_interface::instruction::create_account(
            &payer.pubkey(),
            &context.pubkey(),
            lamports,
            context_len as u64,
            &zkif::ID,
        )],
        payer,
        &[&context, payer],
        false,
    );
    let ix = instruction_kind.encode_verify_proof(
        Some(ContextStateInfo {
            context_state_account: &context.pubkey(),
            context_state_authority: &payer.pubkey(),
        }),
        proof,
    );
    utils::send_tx(
        svm,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(400_000),
            ix,
        ],
        payer,
        &[payer],
        false,
    );
    context.pubkey()
}

/// Close proof context accounts and return their rent to the payer.
///
/// Each context account holds real lamports. Leaving them open leaks rent on
/// every confidential transfer, which adds up fast for an active account.
/// Closing is part of the flow, not an optimization.
pub fn close_contexts(svm: &mut LiteSVM, payer: &Keypair, contexts: &[Pubkey]) -> u64 {
    let before = svm.get_balance(&payer.pubkey()).unwrap();
    let ixs: Vec<Instruction> = contexts
        .iter()
        .map(|c| {
            close_context_state(
                ContextStateInfo {
                    context_state_account: c,
                    context_state_authority: &payer.pubkey(),
                },
                &payer.pubkey(),
            )
        })
        .collect();
    utils::send_tx(svm, &ixs, payer, &[payer], false);

    for c in contexts {
        assert!(svm
            .get_account(c)
            .map(|a| a.data.is_empty())
            .unwrap_or(true));
    }
    svm.get_balance(&payer.pubkey())
        .unwrap()
        .saturating_sub(before)
}

pub fn approve_account(
    svm: &mut LiteSVM,
    holder: &Holder,
    authority: &Keypair,
    mint: Pubkey,
    config: Pubkey,
) {
    let ix = Instruction {
        program_id: confidential_22::ID,
        accounts: confidential_22::accounts::ApproveAccount {
            ata: holder.account,
            config,
            mint,
            authority: authority.pubkey(),
            token_program: t22new::ID,
        }
        .to_account_metas(None),
        data: confidential_22::instruction::ApproveAccount {}.data(),
    };

    utils::send_tx(svm, &[ix], authority, &[authority], false);
}

pub fn create_transfer_ix(
    from: &Keypair,
    config: Pubkey,
    mint: &Keypair,
    from_ata: Pubkey,
    to_ata: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        confidential_22::ID,
        &confidential_22::instruction::Transfer { amount }.data(),
        confidential_22::accounts::Transfer {
            from: from.pubkey(),
            config,
            mint: mint.pubkey(),
            from_ata,
            to_ata,
            token_program: t22new::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}
