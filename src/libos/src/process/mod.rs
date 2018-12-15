pub use self::process::{Process, ProcessRef, Status, pid_t};
pub use self::task::{get_current, run_task};
pub mod table {
    pub use super::process_table::{get};
}
pub use self::spawn::{do_spawn};

pub fn do_getpid() -> pid_t {
    let current_ref = get_current();
    let current_process = current_ref.lock().unwrap();
    current_process.get_pid()
}

pub fn do_exit(exit_code: i32) {
    let current_ref = get_current();
    let mut current_process = current_ref.lock().unwrap();
    current_process.exit(exit_code);
}

pub fn do_wait4(child_pid: u32) -> Result<i32, Error> {
    let child_process = process_table::get(child_pid)
        .ok_or_else(|| (Errno::ECHILD, "Cannot find child process with the given PID"))?;

    let mut exit_code = 0;
    loop {
        let guard = child_process.lock().unwrap();
        if guard.get_status() == Status::ZOMBIE {
            exit_code = guard.get_exit_code();
            break;
        }
        drop(guard);
    }

    let child_pid = child_process.lock().unwrap().get_pid();
    process_table::remove(child_pid);

    Ok(exit_code)
}

mod task;
mod process;
mod process_table;
mod spawn;

use prelude::*;
