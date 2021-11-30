use super::constants::*;
use super::{sigset_t, SigSet};
use crate::prelude::*;

pub fn do_rt_sigprocmask(
    op_and_set: Option<(MaskOp, &sigset_t)>,
    oldset: Option<&mut sigset_t>,
) -> Result<()> {
    debug!(
        "do_rt_sigprocmask: op_and_set: {:?}, oldset: {:?}",
        op_and_set.map(|(op, set)| (op, SigSet::from_c(*set))),
        oldset
    );

    let thread = current!();
    let old_sig_mask = thread.sig_mask();
    if let Some(oldset) = oldset {
        *oldset = old_sig_mask.to_c();
    }
    if let Some((op, &set)) = op_and_set {
        let set = SigSet::from_c(set);
        let new_sig_mask = match op {
            MaskOp::Block => old_sig_mask | set,
            MaskOp::Unblock => old_sig_mask & !set,
            MaskOp::SetMask => set,
        };
        thread.set_sig_mask(new_sig_mask);
    }
    Ok(())
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u32)]
pub enum MaskOp {
    Block = 0,
    Unblock = 1,
    SetMask = 2,
}

impl MaskOp {
    pub fn from_u32(raw: u32) -> Result<MaskOp> {
        let op = match raw {
            0 => MaskOp::Block,
            1 => MaskOp::Unblock,
            2 => MaskOp::SetMask,
            _ => return_errno!(EINVAL, "invalid mask op"),
        };
        Ok(op)
    }
}
