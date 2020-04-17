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
    let mut sig_mask = thread.sig_mask().write().unwrap();
    if let Some(oldset) = oldset {
        *oldset = sig_mask.to_c();
    }
    if let Some((op, &set)) = op_and_set {
        let set = {
            let mut set = SigSet::from_c(set);
            // According to man pages, "it is not possible to block SIGKILL or SIGSTOP.
            // Attempts to do so are silently ignored."
            set -= SIGKILL;
            set -= SIGSTOP;
            set
        };
        match op {
            MaskOp::Block => {
                *sig_mask |= set;
            }
            MaskOp::Unblock => {
                *sig_mask &= !set;
            }
            MaskOp::SetMask => {
                *sig_mask = set;
            }
        };
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
