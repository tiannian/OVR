use crate::{
    ethvm::{precompile::idx_to_h160, OvrAccount},
    ledger::StateBranch,
};
use evm::{
    executor::stack::{PrecompileFailure, PrecompileOutput},
    ExitSucceed,
};
use once_cell::sync::Lazy;
use precompile_utils::{
    error, Address, EvmDataReader, EvmDataWriter, EvmResult, Gasometer, LogsBuilder,
};
use primitive_types::{H160, H256, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use slices::u8_slice;
use std::result::Result as StdResult;
use vsdb::{BranchName, MapxVs, OrphanVs, Vs};

static OVR_ADDR: Lazy<H160> = Lazy::new(|| Erc20Like::ovr_token().contract_addr);
static OVRG_ADDR: Lazy<H160> = Lazy::new(|| Erc20Like::ovrg_token().contract_addr);

// ERC20 transfer event selector, Keccak256("Transfer(address,address,uint256)"),
// event Transfer(address indexed from, address indexed to, uint256 value).
const TRANSFER_EVENT_SELECTOR: &[u8; 32] =
    u8_slice!("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");

// ERC20 approval event selector, Keccak256("Approval(address,address,uint256)"),
// event Approval(address indexed owner, address indexed spender, uint256 value).
const APPROVAL_EVENT_SELECTOR: &[u8; 32] =
    u8_slice!("0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925");

// The gas used value is obtained according to the standard erc20 call.
// https://github.com/OpenZeppelin/openzeppelin-contracts/blob/v4.3.2/contracts/token/ERC20/ERC20.sol
const GAS_NAME: u64 = 3283;
const GAS_SYMBOL: u64 = 3437;
const GAS_DECIMALS: u64 = 243;
const GAS_TOTAL_SUPPLY: u64 = 1003;
const GAS_BALANCE_OF: u64 = 1350;
const GAS_TRANSFER: u64 = 23661;
const GAS_ALLOWANCE: u64 = 1624;
const GAS_APPROVE: u64 = 20750;
const GAS_TRANSFER_FROM: u64 = 6610;

#[derive(Vs, Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Erc20Like {
    pub(crate) contract_addr: H160,
    pub(crate) name: Vec<u8>,
    pub(crate) symbol: Vec<u8>,
    pub(crate) decimal: u32,
    pub(crate) issue_cap: Option<U256>,
    pub(crate) total_supply: OrphanVs<U256>,
    pub(crate) accounts: MapxVs<H160, OvrAccount>,
    // (addr, key) => value
    pub(crate) storages: MapxVs<(H160, H256), H256>,
    // (owner addr, spender addr) => amount
    pub(crate) allowances: MapxVs<(H160, H160), U256>,
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
        let ver: &[u8; 0] = &[];
        Self {
            name,
            symbol,
            decimal,
            issue_cap,
            total_supply: OrphanVs::new(ver[..].into(), 0u8.into()),
            accounts: MapxVs::new(),
            storages: MapxVs::new(),
            allowances: MapxVs::new(),
            contract_addr,
        }
    }

    #[inline(always)]
    pub(crate) fn ovr_token() -> Self {
        let name: &[u8; 96] = u8_slice!(
            "0x0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000a4f76657265616c69747900000000000000000000000000000000000000000000"
        );
        let symbol: &[u8; 96] = u8_slice!(
            "0x000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000034f56520000000000000000000000000000000000000000000000000000000000"
        );
        let issue_cap = Some((1_000_000_000u128 * 10u128.pow(18)).into());
        let contract_addr = idx_to_h160(0x1001);
        Self::new(name.to_vec(), symbol.to_vec(), 18, issue_cap, contract_addr)
    }

    #[inline(always)]
    pub(crate) fn ovrg_token() -> Self {
        let name: &[u8; 96] = u8_slice!(
            "0x0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000a4f76657265616c69747900000000000000000000000000000000000000000000"
        );
        let symbol: &[u8; 96] = u8_slice!(
            "0x000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000044f56524700000000000000000000000000000000000000000000000000000000"
        );
        let issue_cap = None;
        let contract_addr = idx_to_h160(0x1000); // Compitable with Findora
        Self::new(name.to_vec(), symbol.to_vec(), 18, issue_cap, contract_addr)
    }

    // #[inline(always)]
    // fn is_ovr(&self) -> bool {
    //     self.contract_addr == *OVR_ADDR
    // }

    // #[inline(always)]
    // fn is_ovrg(&self) -> bool {
    //     self.contract_addr == *OVRG_ADDR
    // }

    // #[inline(always)]
    // pub(crate) fn is_meta_token(&self) -> bool {
    //     self.is_ovr() || self.is_ovrg()
    // }

    #[inline(always)]
    pub(crate) fn addr_is_ovr(addr: H160) -> bool {
        addr == *OVR_ADDR
    }

    #[inline(always)]
    pub(crate) fn addr_is_ovrg(addr: H160) -> bool {
        addr == *OVRG_ADDR
    }

    #[inline(always)]
    pub(crate) fn addr_is_meta_token(addr: H160) -> bool {
        Self::addr_is_ovr(addr) || Self::addr_is_ovrg(addr)
    }

    // #[inline(always)]
    // fn incr_nonce(&mut self, addr: H160, b: BranchName) {
    //     if let Some(mut a) = self.accounts.get_by_branch(&addr, b) {
    //         a.nonce += U256::one();
    //         self.accounts.insert_by_branch(addr, a, b).unwrap();
    //     }
    // }

    fn name(
        &self,
        input: EvmDataReader,
        gas_limit: Option<u64>,
    ) -> EvmResult<PrecompileOutput> {
        let mut gasometer = Gasometer::new(gas_limit);
        gasometer.record_cost(GAS_NAME)?;

        input.expect_arguments(0)?;

        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gasometer.used_gas(),
            output: EvmDataWriter::new().write_raw_bytes(&self.name).build(),
            logs: vec![],
        })
    }

    fn symbol(
        &self,
        input: EvmDataReader,
        gas_limit: Option<u64>,
    ) -> EvmResult<PrecompileOutput> {
        let mut gasometer = Gasometer::new(gas_limit);
        gasometer.record_cost(GAS_SYMBOL)?;

        input.expect_arguments(0)?;

        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gasometer.used_gas(),
            output: EvmDataWriter::new().write_raw_bytes(&self.symbol).build(),
            logs: vec![],
        })
    }

    fn decimals(
        &self,
        input: EvmDataReader,
        gas_limit: Option<u64>,
    ) -> EvmResult<PrecompileOutput> {
        let mut gasometer = Gasometer::new(gas_limit);
        gasometer.record_cost(GAS_DECIMALS)?;

        input.expect_arguments(0)?;

        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gasometer.used_gas(),
            output: EvmDataWriter::new().write(self.decimal).build(),
            logs: vec![],
        })
    }

    fn total_supply(
        &self,
        input: EvmDataReader,
        gas_limit: Option<u64>,
        b: BranchName,
    ) -> EvmResult<PrecompileOutput> {
        let mut gasometer = Gasometer::new(gas_limit);
        gasometer.record_cost(GAS_TOTAL_SUPPLY)?;

        input.expect_arguments(0)?;

        let am = pnk!(self.total_supply.get_value_by_branch(b));
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gasometer.used_gas(),
            output: EvmDataWriter::new().write(am).build(),
            logs: vec![],
        })
    }

    fn balance_of(
        &self,
        mut input: EvmDataReader,
        gas_limit: Option<u64>,
        b: BranchName,
    ) -> EvmResult<PrecompileOutput> {
        let mut gasometer = Gasometer::new(gas_limit);
        gasometer.record_cost(GAS_BALANCE_OF)?;

        input.expect_arguments(1)?;

        let owner: H160 = input.read::<Address>()?.into();
        let am = self
            .accounts
            .get_by_branch(&owner, b)
            .map(|a| a.balance)
            .unwrap_or_default();
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gasometer.used_gas(),
            output: EvmDataWriter::new().write(am).build(),
            logs: vec![],
        })
    }

    fn allowance(
        &self,
        mut input: EvmDataReader,
        gas_limit: Option<u64>,
        b: BranchName,
    ) -> EvmResult<PrecompileOutput> {
        let mut gasometer = Gasometer::new(gas_limit);
        gasometer.record_cost(GAS_ALLOWANCE)?;

        input.expect_arguments(2)?;

        let owner: H160 = input.read::<Address>()?.into();
        let spender: H160 = input.read::<Address>()?.into();

        let amount = self
            .allowances
            .get_by_branch(&(owner, spender), b)
            .unwrap_or_default();

        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gasometer.used_gas(),
            output: EvmDataWriter::new().write(amount).build(),
            logs: vec![],
        })
    }

    fn transfer(
        &mut self,
        caller: H160,
        mut input: EvmDataReader,
        gas_limit: Option<u64>,
        b: BranchName,
    ) -> EvmResult<PrecompileOutput> {
        let mut gasometer = Gasometer::new(gas_limit);
        gasometer.record_cost(GAS_TRANSFER)?;
        gasometer.record_log_costs_manual(3, 32)?;

        input.expect_arguments(2)?;

        let recipient: H160 = input.read::<Address>()?.into();
        if recipient == H160::zero() {
            return Err(error("transfer to the zero address"));
        }
        let amount: U256 = input.read()?;

        let mut c = self
            .accounts
            .get_by_branch(&caller, b)
            .ok_or_else(|| error("Zero balance"))?;
        if c.balance < amount {
            return Err(error("Insufficient balance"));
        }

        let mut r = self
            .accounts
            .get_by_branch(&recipient, b)
            .unwrap_or_default();

        c.nonce += U256::one();
        c.balance -= amount;
        r.balance += amount;

        self.accounts.insert_by_branch(caller, c, b).unwrap();
        self.accounts.insert_by_branch(recipient, r, b).unwrap();

        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gasometer.used_gas(),
            output: EvmDataWriter::new().write(true).build(),
            logs: LogsBuilder::new(self.contract_addr)
                .log3(
                    TRANSFER_EVENT_SELECTOR,
                    caller,
                    recipient,
                    EvmDataWriter::new().write(amount).build(),
                )
                .build(),
        })
    }

    fn transfer_from(
        &mut self,
        caller: H160,
        mut input: EvmDataReader,
        gas_limit: Option<u64>,
        b: BranchName,
    ) -> EvmResult<PrecompileOutput> {
        let mut gasometer = Gasometer::new(gas_limit);
        gasometer.record_cost(GAS_TRANSFER_FROM)?;
        gasometer.record_log_costs_manual(3, 32)?;
        gasometer.record_log_costs_manual(3, 32)?;

        input.expect_arguments(3)?;

        let from: H160 = input.read::<Address>()?.into();
        if from == H160::zero() {
            return Err(error("transfer from the zero address"));
        }
        let recipient: H160 = input.read::<Address>()?.into();
        if recipient == H160::zero() {
            return Err(error("transfer to the zero address"));
        }
        let amount: U256 = input.read()?;

        let mut allowance = self
            .allowances
            .get_by_branch(&(caller, from), b)
            .unwrap_or_default();
        if allowance < amount {
            return Err(error("transfer amount exceeds allowance"));
        }
        let mut c = self.accounts.get_by_branch(&caller, b).unwrap_or_default();
        let mut f = self
            .accounts
            .get_by_branch(&from, b)
            .ok_or_else(|| error("Zero balance"))?;
        if f.balance < amount {
            return Err(error("Insufficient balance"));
        }
        let mut r = self
            .accounts
            .get_by_branch(&recipient, b)
            .unwrap_or_default();

        c.nonce += U256::one();
        f.balance -= amount;
        r.balance += amount;
        allowance -= amount;

        self.accounts.insert_by_branch(caller, c, b).unwrap();
        self.accounts.insert_by_branch(from, f, b).unwrap();
        self.accounts.insert_by_branch(recipient, r, b).unwrap();
        self.allowances
            .insert_by_branch((from, caller), allowance, b)
            .unwrap();

        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gasometer.used_gas(),
            output: EvmDataWriter::new().write(true).build(),
            logs: LogsBuilder::new(self.contract_addr)
                .log3(
                    TRANSFER_EVENT_SELECTOR,
                    from,
                    recipient,
                    EvmDataWriter::new().write(amount).build(),
                )
                .log3(
                    APPROVAL_EVENT_SELECTOR,
                    from,
                    caller,
                    EvmDataWriter::new().write(allowance - amount).build(),
                )
                .build(),
        })
    }

    fn approve(
        &mut self,
        caller: H160,
        mut input: EvmDataReader,
        gas_limit: Option<u64>,
        b: BranchName,
    ) -> EvmResult<PrecompileOutput> {
        let mut gasometer = Gasometer::new(gas_limit);
        gasometer.record_cost(GAS_APPROVE)?;
        gasometer.record_log_costs_manual(3, 32)?;

        input.expect_arguments(2)?;

        let spender: H160 = input.read::<Address>()?.into();
        if spender == H160::zero() {
            return Err(error("approve to the zero address"));
        }
        let amount: U256 = input.read()?;

        let mut c = self.accounts.get_by_branch(&caller, b).unwrap_or_default();
        let mut allowance = self
            .allowances
            .get_by_branch(&(caller, spender), b)
            .unwrap_or_default();

        c.nonce += U256::one();
        allowance = allowance.saturating_add(amount);

        self.accounts.insert_by_branch(caller, c, b).unwrap();
        self.allowances
            .insert_by_branch((caller, spender), allowance, b)
            .unwrap();

        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            cost: gasometer.used_gas(),
            output: EvmDataWriter::new().write(true).build(),
            logs: LogsBuilder::new(self.contract_addr)
                .log3(
                    APPROVAL_EVENT_SELECTOR,
                    caller,
                    spender,
                    EvmDataWriter::new().write(amount).build(),
                )
                .build(),
        })
    }

    pub(crate) fn execute(
        sb: &mut StateBranch,
        contract_addr: H160,
        caller: H160,
        input: &[u8],
        gas_limit: Option<u64>,
    ) -> StdResult<PrecompileOutput, PrecompileFailure> {
        let mut input = EvmDataReader::new(input);

        let branch = sb.branch.clone();
        let branch = branch.as_slice().into();
        let state = sb.get_evm_state_mut();

        let token_hdr = state.get_token_hdr_mut(contract_addr);

        match &input.read_selector()? {
            Call::Name => token_hdr.name(input, gas_limit),
            Call::Symbol => token_hdr.symbol(input, gas_limit),
            Call::Decimals => token_hdr.decimals(input, gas_limit),
            Call::TotalSupply => token_hdr.total_supply(input, gas_limit, branch),
            Call::BalanceOf => token_hdr.balance_of(input, gas_limit, branch),
            Call::Allowance => token_hdr.allowance(input, gas_limit, branch),

            Call::Transfer => token_hdr.transfer(caller, input, gas_limit, branch),
            Call::TransferFrom => {
                token_hdr.transfer_from(caller, input, gas_limit, branch)
            }
            Call::Approve => token_hdr.approve(caller, input, gas_limit, branch),
        }
    }
}

#[precompile_utils::generate_function_selector]
#[derive(Debug, PartialEq, num_enum::TryFromPrimitive, num_enum::IntoPrimitive)]
enum Call {
    Name = "name()",
    Symbol = "symbol()",
    Decimals = "decimals()",
    TotalSupply = "totalSupply()",
    BalanceOf = "balanceOf(address)",
    Allowance = "allowance(address,address)",

    Transfer = "transfer(address,uint256)",
    TransferFrom = "transferFrom(address,address,uint256)",
    Approve = "approve(address,uint256)",
}
