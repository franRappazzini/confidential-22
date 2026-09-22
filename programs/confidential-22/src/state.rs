use anchor_lang::prelude::*;

use crate::DISCRIMINATOR_SIZE;

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub bump: u8,
}

impl Config {
    pub const SIZE: usize = DISCRIMINATOR_SIZE + Config::INIT_SPACE;
}
