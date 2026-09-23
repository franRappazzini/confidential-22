# Confidential Token

## Program summary

- initialize
- transfer
- unfreeze
- deposit_confidential
- apply_pending_balance
- approve_account

## Run the project with Anchor

### 1) Install prerequisites

Make sure you have the following installed:

- Rust
- Solana CLI
- Anchor CLI

### 2) Run the local test

```bash
# Sync anchor keys
anchor keys sync

# Build the program
anchor build

# Run tests
anchor test
```

## Useful notes

- The program ID is defined in `Anchor.toml` and `programs/confidential-22/src/lib.rs`.
- The project uses `localnet` by default and expects a local validator to be running before `anchor test`.
- If you need to reset the local environment, stop the validator and start it again before deploying.
