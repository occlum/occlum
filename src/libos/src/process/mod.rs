pub use self::arch_prctl::{do_arch_prctl, ArchPrctlCode};
pub use self::exit::{do_exit, do_wait4, ChildProcessFilter};
pub use self::futex::{
    futex_op_and_flags_from_u32, futex_requeue, futex_wait, futex_wake, FutexFlags, FutexOp,
};
pub use self::process::{Status, IDLE_PROCESS};
pub use self::process_table::get;
pub use self::sched::{do_sched_getaffinity, do_sched_setaffinity, do_sched_yield, CpuSet};
pub use self::spawn::{do_spawn, do_spawn_without_exec, ElfFile, FileAction, ProgramHeaderExt};
pub use self::task::{current_pid, get_current, run_task, Task};
pub use self::thread::{do_clone, do_set_tid_address, CloneFlags, ThreadGroup};
pub use self::wait::{WaitQueue, Waiter};

#[allow(non_camel_case_types)]
pub type pid_t = u32;

#[derive(Debug)]
pub struct Process {
    task: Task,
    status: Status,
    pid: pid_t,
    pgid: pid_t,
    tgid: pid_t,
    host_tid: pid_t,
    exit_status: i32,
    // TODO: move cwd, root_inode into a FileSystem structure
    // TODO: should cwd be a String or INode?
    cwd: String,
    elf_path: String,
    clear_child_tid: Option<*mut pid_t>,
    parent: Option<ProcessRef>,
    children: Vec<ProcessWeakRef>,
    waiting_children: Option<WaitQueue<ChildProcessFilter, pid_t>>,
    //thread_group: ThreadGroupRef,
    vm: ProcessVMRef,
    file_table: FileTableRef,
    rlimits: ResourceLimitsRef,
}

pub type ProcessRef = Arc<SgxMutex<Process>>;
pub type ProcessWeakRef = std::sync::Weak<SgxMutex<Process>>;
pub type FileTableRef = Arc<SgxMutex<FileTable>>;
pub type ProcessVMRef = Arc<SgxMutex<ProcessVM>>;
pub type ThreadGroupRef = Arc<SgxMutex<ThreadGroup>>;

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

mod arch_prctl;
mod exit;
mod futex;
mod process;
mod process_table;
mod sched;
mod spawn;
mod task;
mod thread;
mod wait;

use super::*;
use fs::{File, FileRef, FileTable};
use misc::ResourceLimitsRef;
use time::GLOBAL_PROFILER;
use vm::ProcessVM;
