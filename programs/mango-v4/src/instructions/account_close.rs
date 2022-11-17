use anchor_lang::prelude::*;
use anchor_spl::token::Token;

use crate::error::*;
use crate::state::*;

#[derive(Accounts)]
pub struct AccountClose<'info> {
    pub group: AccountLoader<'info, Group>,

    #[account(
        mut,
        has_one = group,
        has_one = owner,
        close = sol_destination
    )]
    pub account: AccountLoaderDynamic<'info, MangoAccount>,
    pub owner: Signer<'info>,

    #[account(mut)]
    /// CHECK: target for account rent needs no checks
    pub sol_destination: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn account_close(ctx: Context<AccountClose>) -> Result<()> {
    let group = ctx.accounts.group.load()?;

    let account = ctx.accounts.account.load_mut()?;

    // don't perform checks if group is just testing
    if !group.is_testing() {
        require!(!account.fixed.being_liquidated(), MangoError::SomeError);
        for ele in account.all_token_positions() {
            require_eq!(ele.is_active(), false);
        }
        for ele in account.all_serum3_orders() {
            require_eq!(ele.is_active(), false);
        }
        for ele in account.all_perp_positions() {
            require_eq!(ele.is_active(), false);
        }
    }

    Ok(())
}