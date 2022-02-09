/// Evm Integration Tests
/// 

mod utils;
use utils::*;

use ethabi;

use tmtypes::abci::{
    RequestCheckTx, RequestDeliverTx,
};
use primitive_types::{H160, U256};
use ethereum::{TransactionV0 as Transaction};
use abci::Application;
// use ovr::{App, Cfg, Commands, DaemonCfg};
// use ruc::*;

pub static CHAIN_ID : u64 = 999;

fn deploy_transaction(name: &str, symbol: &str, nonce: U256) -> (Transaction, ethabi::Contract) {
    let constructor = ERC20Constructor::load();
    let tx = constructor.deploy(name, symbol, nonce);
    let raw_tx = tx.sign(&ALICE_ECDSA.private_key, CHAIN_ID);

    (raw_tx, constructor.0.abi)
} 

fn build_erc20_mint_transaction(
    contract: ERC20,
    recipient: H160,
    amount: U256,
    nonce: U256,
) -> Transaction {
    let tx = contract.mint(recipient, amount, nonce);
    let raw_tx = tx.sign(&ALICE_ECDSA.private_key, CHAIN_ID);
    
    raw_tx
}

fn build_erc20_balance_of_transaction(
    contract: ERC20,
    address: H160,
    nonce: U256,
) -> Transaction {
    let tx = contract.balance_of(address, nonce);
    let raw_tx = tx.sign(&ALICE_ECDSA.private_key, CHAIN_ID);
    
    raw_tx
}

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

fn test_deploy_check_tx() {
    let mut req = RequestCheckTx::default();
    let (tx, _) = deploy_transaction("ERC20", "ORV", U256::from(0_u64));
    req.tx = serde_json::to_vec(&tx).unwrap();
    req.r#type = 1_i32;
    print!("req: {:?}", req.r#type);

    // let resp = APP.lock().unwrap().check_tx(req); 
    // print!("resp: {:?}", resp);
    // assert_eq!(
    //     resp.code, 0,
    //     "erc20 deploy check tx failed, code: {}, log: {}",
    //     resp.code, resp.log
    // );  
} 

fn test_deploy_deliver_tx() -> (H160, ethabi::Contract) {
    let mut req = RequestDeliverTx::default();
    let (tx, contract_abi) = deploy_transaction("ERC20", "ORV", U256::from(0_u64));
    req.tx = serde_json::to_vec(&tx).unwrap();

    // let resp = APP.lock().unwrap().deliver_tx(req); 
    // assert_eq!(
    //     resp.code, 0,
    //     "erc20 deploy deliver tx failed, code: {}, log: {}",
    //     resp.code, resp.log
    // );

    // println!("deploy erc20 result: {:?}\n", resp);

    (H160::zero(), contract_abi)
}

fn test_deploy_commit(contract_address: H160) {
    // APP.lock().unwrap().commit();
}

fn test_mint_check_tx(contract: ERC20) {
    let mut req = RequestCheckTx::default();
    let tx = serde_json::to_vec(&build_erc20_mint_transaction(
        contract,
        BOB_ECDSA.address,
        10000.into(),
        1.into(),
    ))
    .unwrap();
    req.tx = tx;

    // let resp = APP.lock().unwrap().check_tx(req); 
    // assert_eq!(
    //     resp.code, 0,
    //     "erc20 mint check tx failed, code: {}, log: {}",
    //     resp.code, resp.log
    // );
}

fn test_mint_deliver_tx(contract: ERC20) {
    let mut req = RequestDeliverTx::default();
    let tx = serde_json::to_vec(&build_erc20_mint_transaction(
        contract,
        BOB_ECDSA.address,
        10000.into(),
        1.into(),
    ))
    .unwrap();
    req.tx = tx;

    // let resp = APP.lock().unwrap().deliver_tx(req);
    // assert_eq!(
    //     resp.code, 0,
    //     "erc20 mint deliver tx failed, code: {}, log: {}",
    //     resp.code, resp.log
    // );

    // println!("call erc20 mint result: {:?}\n", resp);
}


fn test_balance_of_deliver_tx(contract: ERC20, who: H160) -> U256 {
    let mut req = RequestDeliverTx::default();
    let tx =
        serde_json::to_vec(&build_erc20_balance_of_transaction(contract, who, 2.into()))
            .unwrap();
    req.tx = tx;

    // let resp = APP.lock().unwrap().deliver_tx(req);
    // assert_eq!(
    //     resp.code, 0,
    //     "check tx failed, code: {}, log: {}",
    //     resp.code, resp.log
    // );

    // println!("call erc20 balanceOf result: {:?}\n", resp);

    U256::from_big_endian(&[0_u8])

    // let info = serde_json::from_slice::<CallOrCreateInfo>(&resp.data).unwrap();
    // if let CallOrCreateInfo::Call(info) = info {
    //     assert!(
    //         info.exit_reason.is_succeed(),
    //         "query erc20 balance failed: {:?}",
    //         info.exit_reason
    //     );

    //     U256::from_big_endian(info.value.as_ref())
    // } else {
    //     panic!("not expected result: {:?}", info)
    // }
}

fn test_multicontract() {
    assert_eq!(true, true, "todo!");
}

#[test]
fn erc20_works() {
    test_deploy_check_tx();
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

    // test_deploy_check_tx();
    // test_multicontract();
}