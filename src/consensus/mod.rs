//!
//! # Tendermint ABCI workflow
//!

#![allow(warnings)]

use crate::{
    cfg::DaemonCfg as Cfg,
    common::{BlockHeight, HashValue},
    ethvm::OvrAccount,
    ledger::Ledger,
    tx::Tx,
};
use abci::Application;
use primitive_types::{H160, U256};
use ruc::*;
use std::collections::BTreeMap;
use tmtypes::abci::{
    RequestBeginBlock, RequestCheckTx, RequestDeliverTx, RequestEndBlock, RequestInfo,
    RequestInitChain, ResponseBeginBlock, ResponseCheckTx, ResponseCommit,
    ResponseDeliverTx, ResponseEndBlock, ResponseInfo, ResponseInitChain,
};
use vsdb::MapxOrd;

#[derive(Clone)]
pub struct App {
    ledger: Ledger,
    pub cfg: Cfg,
}

impl App {
    fn new(cfg: Cfg) -> Result<Self> {
        let ledger = Ledger::new(
            cfg.chain_id,
            cfg.chain_name.clone(),
            cfg.chain_version.clone(),
            cfg.gas_price,
            cfg.block_gas_limit,
            cfg.block_base_fee_per_gas,
        )
        .c(d!())?;

        Ok(Self { ledger, cfg })
    }

    pub fn load_or_create(cfg: Cfg) -> Result<Self> {
        cfg.set_vsdb_base_dir().c(d!())?;

        if let Some(ledger) = Ledger::load_from_snapshot().c(d!())? {
            Ok(Self { ledger, cfg })
        } else {
            Self::new(cfg).c(d!())
        }
    }

    #[inline(always)]
    #[cfg(target_os = "linux")]
    fn btm_snapshot(&self, height: BlockHeight) -> Result<()> {
        self.cfg.snapshot(height).c(d!())
    }

    #[inline(always)]
    #[cfg(not(target_os = "linux"))]
    fn btm_snapshot(&self, _: BlockHeight) -> Result<()> {
        Ok(())
    }
}

impl Application for App {
    fn info(&self, req: RequestInfo) -> ResponseInfo {
        let mut resp = ResponseInfo::default();

        let ledger = self.ledger.main.read();

        let b = ledger.last_block().unwrap_or_default();
        let h = b.header.height as i64;

        resp.last_block_height = h;
        if 0 < h {
            resp.last_block_app_hash = b.header_hash;
        }

        println!("\n\n");
        println!("==========================================");
        println!("======== Last committed height: {} ========", h);
        println!("==========================================");
        println!("\n\n");

        resp
    }

    fn init_chain(&self, req: RequestInitChain) -> ResponseInitChain {
        let token_distribution = pnk!(serde_json::from_slice::<BTreeMap<H160, U256>>(
            &req.app_state_bytes
        ));

        for (addr, am) in token_distribution.into_iter() {
            pnk!(
                self.ledger
                    .state
                    .evm
                    .OFUEL
                    .accounts
                    .insert(addr, OvrAccount::from_balance(am))
            );
        }

        ResponseInitChain::default()
    }

    fn check_tx(&self, req: RequestCheckTx) -> ResponseCheckTx {
        let mut resp = ResponseCheckTx::default();
        alt!(0 != req.r#type, return resp);

        if let Ok(tx) = Tx::deserialize(&req.tx) {
            if tx.valid_in_abci() {
                let mut sb = self.ledger.check_tx.write();
                if let Err(e) = info!(sb.apply_tx(tx)) {
                    resp.log = e.to_string();
                    resp.code = 1;
                }
            } else {
                resp.log = "Should not appear in ABCI".to_owned();
                resp.code = 1;
            }
        } else {
            resp.log = "Invalid format".to_owned();
            resp.code = 1;
        }

        resp
    }

    fn begin_block(&self, req: RequestBeginBlock) -> ResponseBeginBlock {
        let header = req.header.unwrap();
        let height = header.height as u64;
        let ts = header.time.unwrap().seconds as u64;

        pnk!(self.ledger.consensus_refresh(header.proposer_address, ts));

        info_omit!(self.btm_snapshot(height));

        ResponseBeginBlock::default()
    }

    fn deliver_tx(&self, req: RequestDeliverTx) -> ResponseDeliverTx {
        let mut resp = ResponseDeliverTx::default();

        if let Ok(tx) = Tx::deserialize(&req.tx) {
            if tx.valid_in_abci() {
                let mut sb = self.ledger.deliver_tx.write();
                if let Err(e) = info!(sb.apply_tx(tx)) {
                    resp.log = e.to_string();
                    resp.code = 1;
                }
            } else {
                resp.log = "Should not appear in ABCI".to_owned();
                resp.code = 1;
            }
        } else {
            resp.log = "Invalid data format".to_owned();
            resp.code = 1;
        }

        resp
    }

    // TODO: staking related logic
    fn end_block(&self, _req: RequestEndBlock) -> ResponseEndBlock {
        ResponseEndBlock::default()
    }

    fn commit(&self) -> ResponseCommit {
        pnk!(self.ledger.commit());

        let mut r = ResponseCommit::default();
        r.data = self.ledger.main.read().last_block_hash();
        r
    }
}
