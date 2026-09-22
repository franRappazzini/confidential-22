use anchor_lang::prelude::*;

pub const DISCRIMINATOR_SIZE: usize = 8;

#[constant]
pub const CONFIG_SEED: &[u8] = b"config";
