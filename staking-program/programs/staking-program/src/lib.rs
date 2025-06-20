use anchor_lang::prelude::*;
use anchor_lang::system_program::Transfer;

declare_id!("81N6Vk2arpbt4vsqzYUqLwqJ8BshMh5fsceN5RhEkGS");

const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
const SECONDS_PER_DAY: u64 = 86_400;
const POINTS_PER_SOL_PER_DAY: u64 = 1_000_000;
#[program]
pub mod staking_program {

    use anchor_lang::system_program::transfer;

    use super::*;

    pub fn create_stake_account(ctx: Context<CreatePdaAccount>) -> Result<()> {
        let stake_account = &mut ctx.accounts.pda_account;
        let clock = Clock::get()?;

        stake_account.owner = ctx.accounts.payer.key();
        stake_account.staked_amount = 0;
        stake_account.total_point = 0;
        stake_account.last_updated_time = clock.unix_timestamp;
        stake_account.bump = ctx.bumps.pda_account;

        msg!("--------------------- stake account is created successfully ---------------------");

        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        require!(amount > 0, StakeError::InvalidAmount);

        let stake_account = &mut ctx.accounts.pda_account;
        let clock = Clock::get()?;

        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: stake_account.to_account_info(),
            },
        );
        transfer(cpi_context, amount)?;

        // Update stake amount
        stake_account.staked_amount = stake_account
            .staked_amount
            .checked_add(amount) // checked_add() for safe arithmetic
            .ok_or(StakeError::Overflow)?;

        msg!(
            "Staked lamports: {}, Total staked: {}, Total points: {}",
            amount,
            stake_account.staked_amount,
            stake_account.total_point / 1000000
        );

        msg!("--------------------- amount is stake successfully ---------------------");

        Ok(())
    }
    pub fn unstake(ctx: Context<Unstake>, amount: u64) -> Result<()> {
        require!(amount > 0, StakeError::InvalidAmount);

        let stake_account = &mut ctx.accounts.pda_account;
        let user_key = ctx.accounts.user.key();
        let clock = Clock::get()?;

        require!(
            stake_account.staked_amount >= amount,
            StakeError::InsufficientStake
        );

        let seeds: &[&[u8]] = &[b"staking_program", user_key.as_ref(), &[stake_account.bump]];

        let signer: &[&[&[u8]]] = &[seeds];

        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: stake_account.to_account_info(),
                to: ctx.accounts.user.to_account_info(),
            },
            signer,
        );

        transfer(cpi_context, amount)?;

        stake_account.staked_amount = stake_account
            .staked_amount
            .checked_sub(amount) // checked_sub() for safe arithmetic
            .ok_or(StakeError::Underflow)?;

        msg!(
            "Staked lamports: {}, Total staked: {}, Total points: {}",
            amount,
            stake_account.staked_amount,
            stake_account.total_point / 1000000
        );

        msg!("--------------------- amount is unstake successfully ---------------------");

        Ok(())
    }

    pub fn claim_points(ctx: Context<ClaimPoints>) -> Result<()> {
        let stake_account = &mut ctx.accounts.pda_account;
        let clock = Clock::get()?;

        let claimable_points = stake_account.total_point / 1000000;

        msg!("Total claimable points are: {}", claimable_points);

        stake_account.total_point = 0;

        Ok(())
    }
}

pub fn update_points(stake_account: &mut StakeAccount, current_time: i64) -> Result<()> {
    let time_passed = current_time
        .checked_sub(stake_account.last_updated_time)
        .ok_or(StakeError::InvalidTimeStamp)?;

    if time_passed > 0 && stake_account.staked_amount > 0 {
        let new_points = calculate_new_points(stake_account.staked_amount, time_passed)?;

        stake_account.total_point = stake_account
            .total_point
            .checked_add(new_points)
            .ok_or(StakeError::Overflow)?;
    }

    stake_account.last_updated_time = current_time;

    Ok(())
}

pub fn calculate_new_points(staked_amount: u64, time_passed_in_sec: i64) -> Result<u64> {
    // Points = (staked_amount_in_sol * time_in_days * points_per_sol_per_day)

    let points = (staked_amount as u128)
        .checked_mul(time_passed_in_sec as u128)
        .ok_or(StakeError::Overflow)?
        .checked_mul(POINTS_PER_SOL_PER_DAY as u128)
        .ok_or(StakeError::Overflow)?
        .checked_div(LAMPORTS_PER_SOL as u128)
        .ok_or(StakeError::Overflow)?
        .checked_div(SECONDS_PER_DAY as u128)
        .ok_or(StakeError::Overflow)?;

    Ok(points as u64)
}

#[account]
pub struct StakeAccount {
    pub owner: Pubkey,
    pub staked_amount: u64,
    pub total_point: u64,
    pub last_updated_time: i64,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct CreatePdaAccount<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        payer = payer,
        space = 8 + 32 + 8 + 8 + 8 + 1,
        seeds = [b"staking_program", payer.key().as_ref()],
        bump
    )]
    pub pda_account: Account<'info, StakeAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        seeds = [b"staking_program", user.key().as_ref()],
        bump = pda_account.bump,
        constraint = pda_account.owner == user.key() @ StakeError::Unauthorized,
    )]
    pub pda_account: Account<'info, StakeAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"staking_program", user.key().as_ref()],
        bump = pda_account.bump,
        constraint = pda_account.owner == user.key() @ StakeError::Unauthorized
    )]
    pub pda_account: Account<'info, StakeAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimPoints<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"staking_program", user.key().as_ref()],
        bump = pda_account.bump,
        constraint = pda_account.owner == user.key() @ StakeError::Unauthorized
    )]
    pub pda_account: Account<'info, StakeAccount>,
}
#[error_code]
pub enum StakeError {
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Amount is invalid, give correct amount")]
    InvalidAmount,
    #[msg("Amount is overflow")]
    Overflow,
    #[msg("Insufficient stake amount")]
    InsufficientStake,
    #[msg("Amount is underflow")]
    Underflow,
    #[msg("Time stamp is invalid")]
    InvalidTimeStamp,
}
