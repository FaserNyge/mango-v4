use anchor_lang::prelude::*;

use crate::error::MangoError;
use crate::state::*;

#[derive(Accounts)]
pub struct Serum3CloseOpenOrders<'info> {
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group
        // owner is checked at #1
    )]
    pub account: AccountLoaderDynamic<'info, MangoAccount>,
    pub owner: Signer<'info>,

    #[account(
        has_one = group,
        has_one = serum_program,
        has_one = serum_market_external,
    )]
    pub serum_market: AccountLoader<'info, Serum3Market>,
    /// CHECK: The pubkey is checked and then it's passed to the serum cpi
    pub serum_program: UncheckedAccount<'info>,
    /// CHECK: The pubkey is checked and then it's passed to the serum cpi
    pub serum_market_external: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Validated inline by checking against the pubkey stored in the account at #2
    pub open_orders: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: target for account rent needs no checks
    pub sol_destination: UncheckedAccount<'info>,
}

pub fn serum3_close_open_orders(ctx: Context<Serum3CloseOpenOrders>) -> Result<()> {
    //
    // Validation
    //
    let mut account = ctx.accounts.account.load_mut()?;
    // account constraint #1
    require!(
        account.fixed.is_owner_or_delegate(ctx.accounts.owner.key()),
        MangoError::SomeError
    );

    let serum_market = ctx.accounts.serum_market.load()?;

    // Validate open_orders #2
    require!(
        account
            .serum3_orders(serum_market.market_index)?
            .open_orders
            == ctx.accounts.open_orders.key(),
        MangoError::SomeError
    );

    //
    // close OO
    //
    cpi_close_open_orders(ctx.accounts)?;

    // Reduce the in_use_count on the token positions - they no longer need to be forced open.
    // We cannot immediately dust tiny positions because we don't have the banks.
    let (base_position, _) = account.token_position_mut(serum_market.base_token_index)?;
    base_position.in_use_count = base_position.in_use_count.saturating_sub(1);
    let (quote_position, _) = account.token_position_mut(serum_market.quote_token_index)?;
    quote_position.in_use_count = quote_position.in_use_count.saturating_sub(1);

    // Deactivate the serum open orders account itself
    account.deactivate_serum3_orders(serum_market.market_index)?;

    Ok(())
}

fn cpi_close_open_orders(ctx: &Serum3CloseOpenOrders) -> Result<()> {
    use crate::serum3_cpi;
    let group = ctx.group.load()?;
    serum3_cpi::CloseOpenOrders {
        program: ctx.serum_program.to_account_info(),
        market: ctx.serum_market_external.to_account_info(),
        open_orders: ctx.open_orders.to_account_info(),
        open_orders_authority: ctx.group.to_account_info(),
        sol_destination: ctx.sol_destination.to_account_info(),
    }
    .call(&group)
}