///A Solana version of the xSushi contract for STEP
/// https://github.com/sushiswap/sushiswap/blob/master/contracts/SushiBar.sol
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use metaplex_token_metadata::state::Metadata;
use std::convert::TryInto;

#[cfg(not(feature = "local-testing"))]
declare_id!("StKLLTf7CQ9n5BgXPSDXENovLTCuNc7N2ehvTb6JZ5x");
#[cfg(feature = "local-testing")]
declare_id!("TesT35sGptoswsVkcLpUUe6U2iTJZE59on1Jno8Vdpg");

#[cfg(not(feature = "local-testing"))]
pub mod constants {
    pub const STEP_TOKEN_MINT_PUBKEY: &str = "AURYydfxJib1ZkTir1Jn1J9ECYUtjb6rKQVmtYaixWPP";
    pub const STAKING_PDA_SEED: &[u8] = b"staking";
}

#[cfg(feature = "local-testing")]
pub mod constants {
    pub const STEP_TOKEN_MINT_PUBKEY: &str = "teST1ieLrLdr4MJPZ7i8mgSCLQ7rTrPRjNnyFdHFaz9";
    pub const STAKING_PDA_SEED: &[u8] = b"staking";
}

#[program]
pub mod step_staking {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        _nonce_vault: u8,
        _nonce_staking: u8,
        lock_end_date: u64,
    ) -> ProgramResult {
        ctx.accounts.staking_account.initializer_key = *ctx.accounts.initializer.key;
        ctx.accounts.staking_account.lock_end_date = lock_end_date;

        Ok(())
    }

    pub fn update_lock_end_date(
        ctx: Context<UpdateLockEndDate>,
        _nonce_staking: u8,
        new_lock_end_date: u64,
    ) -> ProgramResult {
        ctx.accounts.staking_account.lock_end_date = new_lock_end_date;

        Ok(())
    }

    pub fn toggle_freeze_program(ctx: Context<FreezeProgram>, _nonce_staking: u8) -> ProgramResult {
        ctx.accounts.staking_account.freeze_program = !ctx.accounts.staking_account.freeze_program;

        Ok(())
    }

    pub fn stake(
        ctx: Context<Stake>,
        _nonce_vault: u8,
        _nonce_staking: u8,
        _nonce_user_staking: u8,
        amount: u64,
    ) -> ProgramResult {
        let total_token = ctx.accounts.token_vault.amount;
        let total_x_token = ctx.accounts.staking_account.total_x_token;
        let old_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.staking_account);

        //mint x tokens
        if total_token == 0 || total_x_token == 0 {
            ctx.accounts.staking_account.total_x_token =
                (ctx.accounts.staking_account.total_x_token as u128)
                    .checked_add(amount as u128)
                    .unwrap()
                    .try_into()
                    .unwrap();
            ctx.accounts.user_staking_account.x_token_amount =
                (ctx.accounts.user_staking_account.x_token_amount as u128)
                    .checked_add(amount as u128)
                    .unwrap()
                    .try_into()
                    .unwrap();
        } else {
            let what: u64 = (amount as u128)
                .checked_mul(total_x_token as u128)
                .unwrap()
                .checked_div(total_token as u128)
                .unwrap()
                .try_into()
                .unwrap();

            ctx.accounts.staking_account.total_x_token =
                (ctx.accounts.staking_account.total_x_token as u128)
                    .checked_add(what as u128)
                    .unwrap()
                    .try_into()
                    .unwrap();
            ctx.accounts.user_staking_account.x_token_amount =
                (ctx.accounts.user_staking_account.x_token_amount as u128)
                    .checked_add(what as u128)
                    .unwrap()
                    .try_into()
                    .unwrap();
        }

        //transfer the users tokens to the vault
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.token_from.to_account_info(),
                to: ctx.accounts.token_vault.to_account_info(),
                authority: ctx.accounts.token_from_authority.to_account_info(),
            },
        );
        token::transfer(cpi_ctx, amount)?;

        (&mut ctx.accounts.token_vault).reload()?;

        //plus user staking amount
        ctx.accounts.user_staking_account.amount = (ctx.accounts.user_staking_account.amount
            as u128)
            .checked_add(amount as u128)
            .unwrap()
            .try_into()
            .unwrap();

        let new_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.staking_account);

        emit!(PriceChange {
            old_step_per_xstep_e9: old_price.0,
            old_step_per_xstep: old_price.1,
            new_step_per_xstep_e9: new_price.0,
            new_step_per_xstep: new_price.1,
        });

        Ok(())
    }

    pub fn unstake(
        ctx: Context<Unstake>,
        nonce_vault: u8,
        _nonce_staking: u8,
        _nonce_user_staking: u8,
        amount: u64,
    ) -> ProgramResult {
        let now_ts = Clock::get().unwrap().unix_timestamp;
        let lock_end_date = ctx.accounts.staking_account.lock_end_date;

        if (now_ts as u64) < lock_end_date {
            return Err(ErrorCode::NotExceedLockEndDate.into());
        }

        let total_token = ctx.accounts.token_vault.amount;
        let total_x_token = ctx.accounts.staking_account.total_x_token;
        let old_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.staking_account);

        //burn what is being sent
        ctx.accounts.staking_account.total_x_token = (ctx.accounts.staking_account.total_x_token
            as u128)
            .checked_sub(amount as u128)
            .unwrap()
            .try_into()
            .unwrap();
        ctx.accounts.user_staking_account.x_token_amount =
            (ctx.accounts.user_staking_account.x_token_amount as u128)
                .checked_sub(amount as u128)
                .unwrap()
                .try_into()
                .unwrap();

        //determine user share of vault
        let what: u64 = (amount as u128)
            .checked_mul(total_token as u128)
            .unwrap()
            .checked_div(total_x_token as u128)
            .unwrap()
            .try_into()
            .unwrap();

        //compute vault signer seeds
        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds = &[token_mint_key.as_ref(), &[nonce_vault]];
        let signer = &[&seeds[..]];

        //transfer from vault to user
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.token_vault.to_account_info(),
                to: ctx.accounts.token_to.to_account_info(),
                authority: ctx.accounts.token_vault.to_account_info(),
            },
            signer,
        );
        token::transfer(cpi_ctx, what)?;

        (&mut ctx.accounts.token_vault).reload()?;

        //determine user staking amount
        let new_total_token = ctx.accounts.token_vault.amount;
        let new_total_x_token = ctx.accounts.staking_account.total_x_token;

        if new_total_token == 0 || new_total_x_token == 0 {
            ctx.accounts.user_staking_account.amount = 0;
        } else {
            let new_what: u64 = (ctx.accounts.user_staking_account.x_token_amount as u128)
                .checked_mul(new_total_token as u128)
                .unwrap()
                .checked_div(new_total_x_token as u128)
                .unwrap()
                .try_into()
                .unwrap();

            if new_what < ctx.accounts.user_staking_account.amount {
                ctx.accounts.user_staking_account.amount = new_what;
            }
        }

        let new_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.staking_account);

        emit!(PriceChange {
            old_step_per_xstep_e9: old_price.0,
            old_step_per_xstep: old_price.1,
            new_step_per_xstep_e9: new_price.0,
            new_step_per_xstep: new_price.1,
        });

        Ok(())
    }

    pub fn emit_price(ctx: Context<EmitPrice>) -> ProgramResult {
        let price = get_price(&ctx.accounts.token_vault, &ctx.accounts.staking_account);
        emit!(Price {
            step_per_xstep_e9: price.0,
            step_per_xstep: price.1,
        });
        Ok(())
    }

    pub fn emit_reward(ctx: Context<EmitReward>) -> ProgramResult {
        let total_token = ctx.accounts.token_vault.amount;
        let total_x_token = ctx.accounts.staking_account.total_x_token;
        let reward: u64 = (ctx.accounts.user_staking_account.x_token_amount as u128)
            .checked_mul(total_token as u128)
            .unwrap()
            .checked_div(total_x_token as u128)
            .unwrap()
            .checked_sub(ctx.accounts.user_staking_account.amount as u128)
            .unwrap()
            .try_into()
            .unwrap();
        emit!(Reward {
            deposit: ctx.accounts.user_staking_account.amount,
            reward: reward,
        });
        Ok(())
    }

    pub fn stake_nft(ctx: Context<StakeNft>, _nonce_staking: u8) -> ProgramResult {
        let token_metadata = &ctx.accounts.token_metadata;
        let metadata = Metadata::from_account_info(&token_metadata)?;

        match metadata.data.creators {
            Some(creators) => {
                for creator in creators {
                    assert_keys_equal(
                        creator.address,
                        ctx.accounts.staking_account.initializer_key,
                    )?;
                }
            }
            None => {
                msg!("No creators found in metadata");
            }
        }
        Ok(())
    }
}

const E9: u128 = 1000000000;

pub fn get_price<'info>(
    vault: &Account<'info, TokenAccount>,
    staking: &ProgramAccount<'info, StakingAccount>,
) -> (u64, String) {
    let total_token = vault.amount;
    let total_x_token = staking.total_x_token;

    if total_x_token == 0 {
        return (0, String::from("0"));
    }

    let price_uint = (total_token as u128)
        .checked_mul(E9 as u128)
        .unwrap()
        .checked_div(total_x_token as u128)
        .unwrap()
        .try_into()
        .unwrap();
    let price_float = (total_token as f64) / (total_x_token as f64);
    return (price_uint, price_float.to_string());
}

pub fn assert_keys_equal(key1: Pubkey, key2: Pubkey) -> ProgramResult {
    if key1 != key2 {
        Err(ErrorCode::PublicKeyMismatch.into())
    } else {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(_nonce_vault: u8, _nonce_staking: u8)]
pub struct Initialize<'info> {
    #[account(
        address = constants::STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub token_mint: Box<Account<'info, Mint>>,

    #[account(
        init,
        payer = initializer,
        token::mint = token_mint,
        token::authority = token_vault, //the PDA address is both the vault account and the authority (and event the mint authority)
        seeds = [ constants::STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap().as_ref() ],
        bump = _nonce_vault,
    )]
    ///the not-yet-created, derived token vault pubkey
    pub token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        payer = initializer,
        seeds = [ constants::STAKING_PDA_SEED.as_ref() ],
        bump = _nonce_staking,
    )]
    pub staking_account: ProgramAccount<'info, StakingAccount>,

    #[account(mut)]
    ///pays rent on the initializing accounts
    pub initializer: Signer<'info>,

    ///used by anchor for init of the token
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(_nonce_staking: u8)]
pub struct UpdateLockEndDate<'info> {
    pub initializer: Signer<'info>,

    #[account(
        mut,
        seeds = [ constants::STAKING_PDA_SEED.as_ref() ],
        bump = _nonce_staking,
        constraint = staking_account.initializer_key == *initializer.key,
    )]
    pub staking_account: ProgramAccount<'info, StakingAccount>,
}

#[derive(Accounts)]
#[instruction(_nonce_staking: u8)]
pub struct FreezeProgram<'info> {
    pub initializer: Signer<'info>,

    #[account(
        mut,
        seeds = [ constants::STAKING_PDA_SEED.as_ref() ],
        bump = _nonce_staking,
        constraint = staking_account.initializer_key == *initializer.key,
    )]
    pub staking_account: ProgramAccount<'info, StakingAccount>,
}

#[derive(Accounts)]
#[instruction(_nonce_vault: u8, _nonce_staking: u8, _nonce_user_staking: u8)]
pub struct Stake<'info> {
    #[account(
        address = constants::STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub token_mint: Box<Account<'info, Mint>>,

    #[account(mut)]
    //the token account to withdraw from
    pub token_from: Box<Account<'info, TokenAccount>>,

    //the authority allowed to transfer from token_from
    pub token_from_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [ token_mint.key().as_ref() ],
        bump = _nonce_vault,
    )]
    pub token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [ constants::STAKING_PDA_SEED.as_ref() ],
        bump = _nonce_staking,
        constraint = !staking_account.freeze_program,
    )]
    pub staking_account: ProgramAccount<'info, StakingAccount>,

    #[account(
        init_if_needed,
        payer = token_from_authority,
        seeds = [ token_from_authority.key().as_ref() ],
        bump = _nonce_user_staking,
    )]
    pub user_staking_account: ProgramAccount<'info, UserStakingAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(nonce_vault: u8, _nonce_staking: u8, _nonce_user_staking: u8, amount: u64)]
pub struct Unstake<'info> {
    #[account(
        address = constants::STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub token_mint: Box<Account<'info, Mint>>,

    //the authority allowed to transfer from x_token_from
    pub x_token_from_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [ token_mint.key().as_ref() ],
        bump = nonce_vault,
    )]
    pub token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [ constants::STAKING_PDA_SEED.as_ref() ],
        bump = _nonce_staking,
        constraint = !staking_account.freeze_program,
    )]
    pub staking_account: ProgramAccount<'info, StakingAccount>,

    #[account(
        mut,
        seeds = [ x_token_from_authority.key().as_ref() ],
        bump = _nonce_user_staking,
        constraint = user_staking_account.x_token_amount >= amount
    )]
    pub user_staking_account: ProgramAccount<'info, UserStakingAccount>,

    #[account(mut)]
    //the token account to send token
    pub token_to: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct EmitPrice<'info> {
    #[account(
        address = constants::STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub token_mint: Box<Account<'info, Mint>>,

    #[account(
        seeds = [ token_mint.key().as_ref() ],
        bump,
    )]
    pub token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        seeds = [ constants::STAKING_PDA_SEED.as_ref() ],
        bump,
    )]
    pub staking_account: ProgramAccount<'info, StakingAccount>,
}

#[derive(Accounts)]
pub struct EmitReward<'info> {
    #[account(
        address = constants::STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub token_mint: Box<Account<'info, Mint>>,

    #[account(
        seeds = [ token_mint.key().as_ref() ],
        bump,
    )]
    pub token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        seeds = [ constants::STAKING_PDA_SEED.as_ref() ],
        bump,
    )]
    pub staking_account: ProgramAccount<'info, StakingAccount>,

    pub token_from_authority: AccountInfo<'info>,

    #[account(
        seeds = [ token_from_authority.key().as_ref() ],
        bump,
    )]
    pub user_staking_account: ProgramAccount<'info, UserStakingAccount>,
}

#[derive(Accounts)]
#[instruction(_nonce_staking: u8)]
pub struct StakeNft<'info> {
    //the authority allowed to transfer from token_from
    pub token_from_authority: Signer<'info>,

    pub token_metadata: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [ constants::STAKING_PDA_SEED.as_ref() ],
        bump = _nonce_staking,
        constraint = !staking_account.freeze_program,
    )]
    pub staking_account: ProgramAccount<'info, StakingAccount>,
}

#[account]
#[derive(Default)]
pub struct StakingAccount {
    pub initializer_key: Pubkey,
    pub lock_end_date: u64,
    pub total_x_token: u64,
    pub freeze_program: bool,
}

#[account]
#[derive(Default)]
pub struct UserStakingAccount {
    pub amount: u64,
    pub x_token_amount: u64,
}

#[event]
pub struct PriceChange {
    pub old_step_per_xstep_e9: u64,
    pub old_step_per_xstep: String,
    pub new_step_per_xstep_e9: u64,
    pub new_step_per_xstep: String,
}

#[event]
pub struct Price {
    pub step_per_xstep_e9: u64,
    pub step_per_xstep: String,
}

#[event]
pub struct Reward {
    pub deposit: u64,
    pub reward: u64,
}

#[error]
pub enum ErrorCode {
    #[msg("Not exceed lock end date")]
    NotExceedLockEndDate,
    #[msg("PublicKeyMismatch")]
    PublicKeyMismatch,
}
