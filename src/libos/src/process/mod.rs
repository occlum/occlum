pub use self::process::{Status, IDLE_PROCESS};
pub use self::task::{get_current, run_task};
pub use self::process_table::{get};
pub use self::exit::{do_exit, do_wait4, ChildProcessFilter};
pub use self::spawn::{do_spawn, FileAction};
pub use self::wait::{WaitQueue, Waiter};
pub use self::thread::{do_clone, CloneFlags, ThreadGroup, do_set_tid_address};
pub use self::futex::{FutexOp, FutexFlags, futex_op_and_flags_from_u32, futex_wake, futex_wait};
pub use self::arch_prctl::{ArchPrctlCode, do_arch_prctl};

#[allow(non_camel_case_types)]
pub type pid_t = u32;

#[derive(Debug)]
pub struct Process {
    task: Task,
    status: Status,
    pid: pid_t,
    pgid: pid_t,
    tgid: pid_t,
    exit_status: i32,
    // TODO: move cwd, root_inode into a FileSystem structure
    // TODO: should cwd be a String or INode?
    cwd: String,
    clear_child_tid: Option<*mut pid_t>,
    parent: Option<ProcessRef>,
    children: Vec<ProcessWeakRef>,
    waiting_children: Option<WaitQueue<ChildProcessFilter, pid_t>>,
    vm: ProcessVMRef,
    file_table: FileTableRef,
    rlimits: ResourceLimitsRef,
}

pub type ProcessRef = Arc<SgxMutex<Process>>;
pub type ProcessWeakRef = std::sync::Weak<SgxMutex<Process>>;
pub type FileTableRef = Arc<SgxMutex<FileTable>>;
pub type ProcessVMRef = Arc<SgxMutex<ProcessVM>>;

pub fn do_getpid() -> pid_t {
    let current_ref = get_current();
    let current = current_ref.lock().unwrap();
    current.get_pid()
}

pub fn do_gettid() -> pid_t {
    let current_ref = get_current();
    let current = current_ref.lock().unwrap();
    current.get_tid()
}

pub fn do_getpgid() -> pid_t {
    let current_ref = get_current();
    let current = current_ref.lock().unwrap();
    current.get_pgid()
}

pub fn do_getppid() -> pid_t {
    let parent_ref = {
        let current_ref = get_current();
        let current = current_ref.lock().unwrap();
        current.get_parent().clone()
    };
    let parent = parent_ref.lock().unwrap();
    parent.get_pid()
}

mod exit;
mod process;
mod process_table;
mod spawn;
mod task;
mod wait;
mod thread;
mod futex;
mod arch_prctl;

use self::task::Task;
use super::*;
use fs::{File, FileRef, FileTable};
use vm::{ProcessVM, VMRangeTrait};
use misc::{ResourceLimitsRef};
