use anchor_lang::prelude::*;
use marinade_sdk::state::marinade::Marinade;
use prog_dynamic_amm::constants::depeg;
use std::convert::TryInto;

pub fn get_virtual_price(bytes: &[u8]) -> Option<u64> {
    let stake_state = Marinade::deserialize(&mut &bytes[8..]).ok()?;

    let virtual_price = (stake_state.msol_price as u128)
        .checked_mul(depeg::PRECISION as u128)?
        .checked_div(Marinade::PRICE_DENOMINATOR as u128)?;

    virtual_price.try_into().ok()
}

pub mod stake {
    use super::*;
    declare_id!("8szGkuLTAux9XMgZ2vtY39jVSowEcpBfFfD8hXSEqdGC");
}
