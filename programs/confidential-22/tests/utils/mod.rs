use anchor_spl::token_2022;
use litesvm::LiteSVM;
use litesvm_token::CreateAssociatedTokenAccount;
use solana_keypair::{Address, Keypair};
use solana_message::{Instruction, Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

// Setup function to initialize LiteSVM and create a payer keypair
pub fn setup() -> (LiteSVM, Keypair, Keypair, Keypair) {
    let authority = Keypair::new();
    let user1 = Keypair::new();
    let user2 = Keypair::new();

    let mut svm = LiteSVM::new();

    let program_bytes = include_bytes!("../../../../target/deploy/confidential_22.so");
    svm.add_program(confidential_22::ID, program_bytes).unwrap();

    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&user1.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&user2.pubkey(), 1_000_000_000).unwrap();

    (svm, authority, user1, user2)
}

pub fn send_tx(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
    logs: bool,
) {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    let res = svm.send_transaction(tx);
    match res {
        Err(e) => {
            println!("error logs: {:#?}", e.meta.logs);
            panic!("❌ {}", &e.err)
        }
        Ok(sig) => {
            if logs {
                println!("✅ tx signature: {}", sig.signature)
            }
        }
    }
}

pub fn pdas() -> Address {
    let program_id = confidential_22::ID;

    let config =
        Address::find_program_address(&[confidential_22::constants::CONFIG_SEED], &program_id).0;

    config
}

pub fn token(svm: &mut LiteSVM, user: &Keypair, mint: Address) -> Address {
    let user_ata = CreateAssociatedTokenAccount::new(svm, &user, &mint)
        .owner(&user.pubkey())
        .token_program_id(&token_2022::ID)
        .send()
        .unwrap();

    user_ata
}
