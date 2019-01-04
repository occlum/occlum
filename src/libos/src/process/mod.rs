pub use self::process::{Status, IDLE_PROCESS};
pub use self::task::{get_current, run_task};
pub mod table {
    pub use super::process_table::{get};
}
pub use self::spawn::{do_spawn};
pub use self::exit::{do_exit, do_wait4, ChildProcessFilter};
pub use self::wait::{Waiter, WaitQueue};

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
    children: Vec<ProcessRef>,
    waiting_children: Option<WaitQueue<ChildProcessFilter, pid_t>>,
    vm: ProcessVM,
    file_table: FileTable,
}

pub type ProcessRef = Arc<SgxMutex<Process>>;


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

mod task;
mod process;
mod process_table;
mod spawn;
mod wait;
mod exit;

use prelude::*;
use vm::{ProcessVM, VMRangeTrait};
use fs::{FileTable, File, FileRef};
use self::task::{Task};
