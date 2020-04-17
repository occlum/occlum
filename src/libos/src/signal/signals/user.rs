use super::super::c_types::*;
use super::super::constants::*;
use super::super::{SigNum, Signal};
use crate::prelude::*;

#[derive(Debug, Copy, Clone)]
pub struct UserSignal {
    num: SigNum,
    pid: pid_t, // sender's pid
    uid: uid_t, // sender's uid
    kind: UserSignalKind,
}

#[derive(Debug, Copy, Clone)]
pub enum UserSignalKind {
    Kill,
    Tkill,
    Sigqueue(sigval_t),
}

unsafe impl Sync for UserSignalKind {}
unsafe impl Send for UserSignalKind {}

impl UserSignal {
    pub fn new(num: SigNum, kind: UserSignalKind, pid: pid_t, uid: uid_t) -> Self {
        Self {
            num,
            kind,
            pid,
            uid,
        }
    }

    pub fn pid(&self) -> pid_t {
        self.pid
    }

    pub fn uid(&self) -> uid_t {
        self.uid
    }

    pub fn kind(&self) -> UserSignalKind {
        self.kind
    }
}

impl Signal for UserSignal {
    fn num(&self) -> SigNum {
        self.num
    }

    fn to_info(&self) -> siginfo_t {
        let code = match self.kind {
            UserSignalKind::Kill => SI_USER,
            UserSignalKind::Tkill => SI_TKILL,
            UserSignalKind::Sigqueue(_) => SI_QUEUE,
        };

        let mut info = siginfo_t::new(self.num, code);
        info.set_si_pid(self.pid);
        info.set_si_uid(self.uid);
        if let UserSignalKind::Sigqueue(val) = self.kind {
            info.set_si_value(val);
        }

        info
    }
}
