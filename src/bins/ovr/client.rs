//!
//! Client-end operations
//!

use eth_utils::ecdsa_keys;
use ovr::cfg::CliCfg;
use ruc::*;

pub fn exec(cfg: CliCfg) -> Result<()> {
    if cfg.gen_account {
        gen_account();
        return Ok(());
    }
    todo!()
}

fn gen_account() {
    let (keypair, phrase, _) = ecdsa_keys::SecpPair::generate_with_phrase(None);
    let addr = keypair.address();

    println!("\x1b[31;1mAddress:\x1b[0m {:X}", addr);
    println!("\x1b[31;1mPhrase:\x1b[0m {}", phrase);
}
