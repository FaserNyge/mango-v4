use anchor_lang::prelude::*;

use crate::accounts_ix::*;
use crate::error::MangoError;
use crate::state::*;
use crate::instructions::perp_place_order::*;

// TODO FAS
pub fn perp_cancel_replace_all_orders(ctx: Context<PerpPlaceOrder>, // PerpCancelReplaceAllOrders ?
                                      mut orders: Vec<Order>,
                                      limit: u8
) -> Result<Vec<Option<u128>>> {
    let mut result_order_ids = Vec::new();

    let now_ts: u64 = Clock::get()?.unix_timestamp.try_into().unwrap();
    let oracle_price;

    let mut account = ctx.accounts.account.load_full_mut()?;

    // account constraint #1
    require!(
        account.fixed.is_owner_or_delegate(ctx.accounts.owner.key()),
        MangoError::SomeError
    );

    let mut perp_market = ctx.accounts.perp_market.load_mut()?;
    let mut book = Orderbook {
        bids: ctx.accounts.bids.load_mut()?,
        asks: ctx.accounts.asks.load_mut()?,
    };

    // Update funding if possible.
    //
    // Doing this automatically here makes it impossible for attackers to add orders to the orderbook
    // before triggering the funding computation.
    oracle_price = perp_place_order_update_funding(&ctx, now_ts)?;

    let account_pk = ctx.accounts.account.key();
    let (perp_market_index, settle_token_index) = {
        let perp_market = ctx.accounts.perp_market.load()?;
        (
            perp_market.perp_market_index,
            perp_market.settle_token_index,
        )
    };

    //
    // Create the perp position if needed
    //
    account.ensure_perp_position(perp_market_index, settle_token_index)?;

    //
    // Pre-health computation, _after_ perp position is created
    //
    let pre_health_opt = perp_place_order_pre_health_checks(&ctx, now_ts, &mut account)?;

    let now_ts: u64 = Clock::get()?.unix_timestamp.try_into().unwrap();
    perp_place_order_update_buyback_fees(&ctx, &mut account, now_ts)?;

    //
    // Cancel existing orders
    //
    book.cancel_all_orders(&mut account.borrow_mut(), &mut perp_market, limit, None)?;

    //
    // Place new orders
    //
    let mut event_queue = ctx.accounts.event_queue.load_mut()?;
    for mut order in orders {
        require_gte!(order.max_base_lots, 0);
        require_gte!(order.max_quote_lots, 0);

        let pp = account.perp_position(perp_market_index)?;
        let effective_pos = pp.effective_base_position_lots();

        order.max_base_lots = compute_max_base_lots(&mut order, &mut perp_market, pp)?;

        let order_id_opt = book.new_order(
            order,
            &mut perp_market,
            &mut event_queue,
            oracle_price,
            &mut account.borrow_mut(),
            &account_pk,
            now_ts,
            limit,
        )?;
        
        result_order_ids.push(order_id_opt);
    }

    //
    // Health check
    //
    post_place_order_health_check(&mut account, perp_market_index, pre_health_opt, &mut perp_market)?;

    Ok(result_order_ids)
}
