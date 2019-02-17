pub use self::process::{Status, IDLE_PROCESS};
pub use self::task::{get_current, run_task};
pub mod table {
    pub use super::process_table::get;
}
pub use self::exit::{do_exit, do_wait4, ChildProcessFilter};
pub use self::spawn::{do_spawn, FileAction};
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
    exit_status: i32,
    exec_path: String,
    parent: Option<ProcessRef>,
    children: Vec<ProcessWeakRef>,
    waiting_children: Option<WaitQueue<ChildProcessFilter, pid_t>>,
    vm: ProcessVM,
    file_table: FileTable,
}

pub type ProcessRef = Arc<SgxMutex<Process>>;
pub type ProcessWeakRef = std::sync::Weak<SgxMutex<Process>>;

pub fn do_getpid() -> pid_t {
    let current_ref = get_current();
    let current = current_ref.lock().unwrap();
    current.get_pid()
}

pub fn do_getgpid() -> pid_t {
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

use self::task::Task;
use super::*;
use fs::{File, FileRef, FileTable};
use vm::{ProcessVM, VMRangeTrait};
