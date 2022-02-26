//!
//! Client-end operations
//!

use eth_utils::ecdsa_keys;
use ovr::cfg::CliCfg;
use primitive_types::H160;
use ruc::*;

pub fn exec(cfg: CliCfg) -> Result<()> {
    if cfg.gen_account {
        let (addr, phrase) = gen_account();
        println!("\x1b[31;1mAddress:\x1b[0m 0x{:x}", addr);
        println!("\x1b[31;1mPhrase:\x1b[0m {}", phrase);
        return Ok(());
    }
    todo!()
}

// return: address + phrase
pub fn gen_account() -> (H160, String) {
    let (keypair, phrase, _) = ecdsa_keys::SecpPair::generate_with_phrase(None);
    let addr = keypair.address();
    (addr, phrase)
}
