pub use self::process::{Status, IDLE_PROCESS};
pub use self::task::{get_current, run_task};
pub mod table {
    pub use super::process_table::{get};
}
pub use self::spawn::{do_spawn};


#[allow(non_camel_case_types)]
pub type pid_t = u32;

#[derive(Debug)]
pub struct Process {
    task: Task,
    status: Status,
    pid: pid_t,
    tgid: pid_t,
    exit_status: i32,
    exec_path: String,
    parent: Option<ProcessRef>,
    children: Vec<ProcessRef>,
    vm: ProcessVM,
    file_table: FileTable,
}

pub type ProcessRef = Arc<SgxMutex<Process>>;


pub fn do_getpid() -> pid_t {
    let current_ref = get_current();
    let current = current_ref.lock().unwrap();
    current.get_pid()
}

pub fn do_getppid() -> pid_t {
    let current_ref = get_current();
    let current = current_ref.lock().unwrap();
    let parent_ref = current.get_parent();
    let parent = parent_ref.lock().unwrap();
    parent.get_pid()
}

pub fn do_exit(exit_status: i32) {
    let current_ref = get_current();
    let mut current = current_ref.lock().unwrap();

    current.exit_status = exit_status;
    current.status = Status::ZOMBIE;

    for child_ref in &current.children {
        let mut child = child_ref.lock().unwrap();
        child.parent = Some(IDLE_PROCESS.clone());
    }
    current.children.clear();
}

pub fn do_wait4(child_pid: u32) -> Result<i32, Error> {
    let child_process = process_table::get(child_pid)
        .ok_or_else(|| (Errno::ECHILD, "Cannot find child process with the given PID"))?;

    let mut exit_status = 0;
    loop {
        let guard = child_process.lock().unwrap();
        if guard.get_status() == Status::ZOMBIE {
            exit_status = guard.get_exit_status();
            break;
        }
        drop(guard);
    }

    let child_pid = child_process.lock().unwrap().get_pid();
    process_table::remove(child_pid);

    Ok(exit_status)
}

mod task;
mod process;
mod process_table;
mod spawn;

use prelude::*;
use vm::{ProcessVM, VMRangeTrait};
use fs::{FileTable, File, FileRef};
use self::task::{Task};
