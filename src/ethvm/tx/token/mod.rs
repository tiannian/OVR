use crate::ethvm::{precompile::idx_to_h160, OvrAccount};
use primitive_types::{H160, H256, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use slices::u8_slice;
use vsdb::{MapxDkVs, MapxVs, OrphanVs, Vs};

pub const DECIMAL: u32 = 18;

#[derive(Vs, Clone, Debug, Deserialize, Serialize)]
pub struct Erc20Like {
    // will never change
    pub contract_addr: H160,

    pub name: Vec<u8>,
    pub symbol: Vec<u8>,
    pub decimal: u32,
    pub issue_cap: Option<U256>,
    pub total_supply: OrphanVs<U256>,

    pub accounts: MapxVs<H160, OvrAccount>,

    // (addr, key) => value
    pub storages: MapxDkVs<H160, H256, H256>,

    // (owner addr, spender addr) => amount
    pub allowances: MapxVs<(H160, H160), U256>,
}

impl Erc20Like {
    #[inline(always)]
    fn new(
        name: Vec<u8>,
        symbol: Vec<u8>,
        decimal: u32,
        issue_cap: Option<U256>,
        contract_addr: H160,
    ) -> Self {
        Self {
            name,
            symbol,
            decimal,
            issue_cap,
            total_supply: OrphanVs::new(0u8.into()),
            accounts: MapxVs::new(),
            storages: MapxDkVs::new(),
            allowances: MapxVs::new(),
            contract_addr,
        }
    }

    #[inline(always)]
    pub fn ofuel_token() -> Self {
        let name: &[u8; 96] = u8_slice!(
            "0x0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000a4f76657265616c69747900000000000000000000000000000000000000000000"
        );
        let symbol: &[u8; 96] = u8_slice!(
            "0x000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000054f4655454c000000000000000000000000000000000000000000000000000000"
        );
        let issue_cap = None;
        let contract_addr = idx_to_h160(0x1000); // Compitable with F
        Self::new(
            name.to_vec(),
            symbol.to_vec(),
            DECIMAL,
            issue_cap,
            contract_addr,
        )
    }
}
