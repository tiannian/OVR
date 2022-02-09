/// Evm Integration Tests
/// 

mod utils;
use utils::*;

use ethabi;

use tmtypes::abci::{
    RequestCheckTx,
};
use primitive_types::{H160, U256};

fn deploy_transaction(name: &str, symbol: &str, nonce: U256) -> ethabi::Contract {
    let constructor = ERC20Constructor::load();
    let tx = constructor.deploy(name, symbol, nonce);

    constructor.0.abi
} 

// fn mint(
//     contract: ERC20,
//     recipient: H160,
//     amount: U256,
//     nonce: U256,
// ) {
//     let _tx = contract.mint(recipient, amount, nonce);
// }

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

fn test_deploy_check_tx() {
    let mut req = RequestCheckTx::default();
    let tx =
        serde_json::to_vec(&deploy_transaction("ERC20", "ORV", 0.into()));
}

// fn test_mint_check_tx() {
//     let mut req = RequestCheckTx::default();
//     let tx = serde_json::to_vec(&mint(
//         contract,
//         BOB_ECDSA.address,
//         10000.into(),
//         1.into(),
//     ))
//     .unwrap();
//     req.tx = tx;
// }

fn test_multicontract() {
    assert_eq!(true, true, "todo!");
}

#[test]
fn erc20_works() {
    // test_deploy_check_tx();
    // let (address, abi) = test_deploy_deliver_tx();
    // test_deploy_commit(address);

    // let erc20_instance = ERC20(DeployedContract { abi, address });
    // // erc20 mint
    // test_mint_check_tx(erc20_instance.clone());
    // test_mint_deliver_tx(erc20_instance.clone());
    // assert_eq!(
    //     test_balance_of_deliver_tx(erc20_instance.clone(), BOB_ECDSA.address),
    //     10000.into()
    // );

    test_deploy_check_tx();
    test_multicontract();
}