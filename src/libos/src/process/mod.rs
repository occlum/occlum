/// Process/thread subsystem.
///
/// The subsystem implements process/thread-related system calls, which are
/// mainly based on the three concepts below:
///
/// * [`Process`]. A process has a parent and may have multiple child processes and
/// can own multiple threads.
/// * [`Thread`]. A thread belongs to one and only one process and owns a set
/// of OS resources, e.g., virtual memory, file tables, etc.
/// * [`Task`]. A task belongs to one and only one thread, for which it deals with
/// the low-level details about thread execution.
use crate::fs::{FileRef, FileTable, FsView};
use crate::misc::ResourceLimits;
use crate::prelude::*;
use crate::sched::SchedAgent;
use crate::vm::ProcessVM;

use self::process::{ChildProcessFilter, ProcessBuilder, ProcessInner};
use self::thread::{ThreadBuilder, ThreadId, ThreadInner};
use self::wait::{WaitQueue, Waiter};

pub use self::do_spawn::do_spawn_without_exec;
pub use self::process::{Process, ProcessStatus, IDLE};
pub use self::syscalls::*;
pub use self::task::Task;
pub use self::thread::{Thread, ThreadStatus};

mod do_arch_prctl;
mod do_clone;
mod do_exit;
mod do_futex;
mod do_getpid;
mod do_set_tid_address;
mod do_spawn;
mod do_wait4;
mod process;
mod syscalls;
mod thread;
mod wait;

pub mod current;
pub mod elf_file;
pub mod table;
pub mod task;

#[allow(non_camel_case_types)]
pub type pid_t = u32;

pub type ProcessRef = Arc<Process>;
pub type ThreadRef = Arc<Thread>;
pub type FileTableRef = Arc<SgxMutex<FileTable>>;
pub type ProcessVMRef = Arc<SgxMutex<ProcessVM>>;
pub type FsViewRef = Arc<SgxMutex<FsView>>;
pub type SchedAgentRef = Arc<SgxMutex<SchedAgent>>;
pub type ResourceLimitsRef = Arc<SgxMutex<ResourceLimits>>;
