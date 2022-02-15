use btm::BtmCfg;
use ovr::cfg::{SnapCfg, SnapOps};
use ruc::*;

pub fn exec(cfg: SnapCfg) -> Result<()> {
    let btm_cfg = BtmCfg::try_from(&cfg).c(d!())?;
    match cfg.commands {
        SnapOps::List(_) => btm_cfg.list_snapshots().c(d!()),
        SnapOps::Clean(_) => btm_cfg.clean_snapshots().c(d!()),
        SnapOps::Rollback(args) => btm_cfg.rollback(args.height, args.exact).c(d!()),
    }
}
