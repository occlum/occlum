use super::c_types::{sigaction_t, SIG_DFL, SIG_IGN};
use super::constants::*;
use super::{SigNum, SigSet};
use crate::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SigAction {
    Dfl, // Default action
    Ign, // Ignore this signal
    User {
        // User-given handler
        handler_addr: usize,
        flags: SigActionFlags,
        restorer_addr: usize,
        mask: SigSet,
    },
}

impl Default for SigAction {
    fn default() -> Self {
        SigAction::Dfl
    }
}

impl SigAction {
    pub fn from_c(sa_c: &sigaction_t) -> Result<Self> {
        let sa = match sa_c.handler {
            SIG_DFL => SigAction::Dfl,
            SIG_IGN => SigAction::Ign,
            _ => SigAction::User {
                handler_addr: sa_c.handler as usize,
                flags: SigActionFlags::from_u32(sa_c.flags)?,
                restorer_addr: sa_c.restorer as usize,
                mask: {
                    let mut mask = SigSet::from_c(sa_c.mask);
                    // According to man pages, "it is not possible to block SIGKILL or SIGSTOP.
                    // Attempts to do so are silently ignored."
                    mask -= SIGKILL;
                    mask -= SIGSTOP;
                    mask
                },
            },
        };
        Ok(sa)
    }

    pub fn to_c(&self) -> sigaction_t {
        match self {
            SigAction::Dfl => sigaction_t {
                handler: SIG_DFL,
                flags: 0,
                restorer: std::ptr::null(),
                mask: 0,
            },
            SigAction::Ign => sigaction_t {
                handler: SIG_IGN,
                flags: 0,
                restorer: std::ptr::null(),
                mask: 0,
            },
            SigAction::User {
                handler_addr,
                flags,
                restorer_addr,
                mask,
            } => sigaction_t {
                handler: *handler_addr as *const c_void,
                flags: flags.to_u32(),
                restorer: *restorer_addr as *mut c_void,
                mask: mask.to_c(),
            },
        }
    }
}

bitflags! {
    pub struct SigActionFlags: u32 {
        const SA_NOCLDSTOP  = 1;
        const SA_NOCLDWAIT  = 2;
        const SA_SIGINFO    = 4;
        const SA_ONSTACK    = 0x08000000;
        const SA_RESTART    = 0x10000000;
        const SA_NODEFER    = 0x40000000;
        const SA_RESETHAND  = 0x80000000;
        const SA_RESTORER   = 0x04000000;
    }
}

impl SigActionFlags {
    pub fn from_u32(bits: u32) -> Result<SigActionFlags> {
        let flags =
            Self::from_bits(bits).ok_or_else(|| errno!(EINVAL, "invalid sigaction flags"))?;
        if flags.contains(SigActionFlags::SA_RESTART) {
            warn!("SA_RESTART is not supported");
        }
        Ok(flags)
    }

    pub fn to_u32(&self) -> u32 {
        self.bits()
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SigDefaultAction {
    Term, // Default action is to terminate the process.
    Ign,  // Default action is to ignore the signal.
    Core, // Default action is to terminate the process and dump core (see core(5)).
    Stop, // Default action is to stop the process.
    Cont, // Default action is to continue the process if it is currently stopped.
}

impl SigDefaultAction {
    pub fn from_signum(num: SigNum) -> SigDefaultAction {
        match num {
            SIGABRT | // = SIGIOT
            SIGBUS  |
            SIGFPE  |
            SIGILL  |
            SIGQUIT |
            SIGSEGV |
            SIGSYS  | // = SIGUNUSED
            SIGTRAP |
            SIGXCPU |
            SIGXFSZ
                => SigDefaultAction::Core,
            SIGCHLD |
            SIGURG  |
            SIGWINCH
                => SigDefaultAction::Ign,
            SIGCONT
                => SigDefaultAction::Cont,
            SIGSTOP |
            SIGTSTP |
            SIGTTIN |
            SIGTTOU
                => SigDefaultAction::Stop,
            _
                => SigDefaultAction::Term,
        }
    }
}
