use crate::ledger::StateBranch;
use ethereum::TransactionAny;
use ruc::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tx {
    tx: TransactionAny,
}

impl Tx {
    pub(crate) fn apply(self, sb: &mut StateBranch) -> Result<()> {
        self.pre_exec()
            .c(d!())
            .and_then(|_| self.exec().c(d!()))
            .and_then(|_| self.post_exec().c(d!()))
    }

    // - verify the transaction signature
    // - ensure the transaction nonce is bigger than the last nonce
    // - ensure the balance of OVRg is bigger than `spent_amount + gas_limit`
    // - deducte `gas_limit` from the balance of OVRg
    fn pre_exec(&self) -> Result<()> {
        todo!()
    }

    // Support:
    // - Legacy transactions
    // - EIP2930 transactons
    // - EIP1559 transactions
    //
    // Both LegacyTransaction and TransactionV2 data formats are supported.
    fn exec(&self) -> Result<()> {
        todo!()
    }

    // refund the remaining gas
    fn post_exec(&self) -> Result<()> {
        todo!()
    }
}
