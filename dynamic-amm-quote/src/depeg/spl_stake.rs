use borsh::BorshDeserialize;
use prog_dynamic_amm::constants::depeg;
use spl_stake_pool::state::StakePool;
use std::convert::TryInto;

pub fn get_virtual_price(bytes: &[u8]) -> Option<u64> {
    let stake = StakePool::try_from_slice(bytes).ok()?;

    let virtual_price = (stake.total_lamports as u128)
        .checked_mul(depeg::PRECISION as u128)?
        .checked_div(stake.pool_token_supply as u128)?;

    virtual_price.try_into().ok()
}
