use super::{ProcessRef, ThreadId, ThreadRef};
use crate::fs::FileTable;
use crate::prelude::*;
use crate::syscall::CpuContext;
use std::collections::HashMap;
use std::mem;

// From Man page: The calling thread is suspended until the child terminates (either normally, by calling
// _exit(2), or abnormally, after delivery of a fatal signal), or it makes a call to execve(2).
// Until that point, the child shares all memory with its parent, including the stack.
//
// Thus in this implementation, the main idea is to let child use parent's task until exit or execve.
//
// Limitation:
// The child process will not have a complete process structure before execve. Thus during the time from vfork
// to new child process execve or exit, the child process just reuse the parent process's everything, including
// task, pid and etc. And also the log of child process will not start from the point that vfork returns but the
// point that execve returns.

lazy_static! {
    // Store all the parents's file tables who call vfork. It will be recovered when the child exits or has its own task.
    // K: parent pid, V: parent file table
    static ref VFORK_PARENT_FILE_TABLES: SgxMutex<HashMap<pid_t, FileTable>> = SgxMutex::new(HashMap::new());
}

thread_local! {
    // Store the current process' vforked child and current thread's cpu context. A parent only has one vforked child at a time.
    static VFORK_CONTEXT: RefCell<Option<(pid_t, CpuContext)>> = Default::default();
}

pub fn do_vfork(mut context: *mut CpuContext) -> Result<isize> {
    let current = current!();
    trace!("vfork parent process pid = {:?}", current.process().pid());

    // Generate a new pid for child process
    let child_pid = {
        let new_tid = ThreadId::new();
        new_tid.as_u32() as pid_t
    };

    // Save parent's context in TLS
    VFORK_CONTEXT.with(|cell| {
        let mut ctx = cell.borrow_mut();
        let new_context = (child_pid, unsafe { (*context).clone() });
        *ctx = Some(new_context);
    });

    // Save parent's file table
    let parent_pid = current.process().pid();
    let mut vfork_file_tables = VFORK_PARENT_FILE_TABLES.lock().unwrap();
    let parent_file_table = {
        let mut current_file_table = current.files().lock().unwrap();
        let new_file_table = current_file_table.clone();
        // FileTable contains non-cloned struct, so here we do a memory replacement to use new
        // file table in child and store the original file table in TLS.
        mem::replace(&mut *current_file_table, new_file_table)
    };
    if let Some(_) = vfork_file_tables.insert(parent_pid, parent_file_table) {
        return_errno!(EINVAL, "current process's vfork has not returned yet");
    }

    // This is the first time return and will return as child.
    // The second time return will return as parent in vfork_return_to_parent.
    info!("vfork child pid = {:?}", child_pid);
    return Ok(0 as isize);
}

// Check if the calling process is a vforked child process that reuse parent's task and pid.
pub fn is_vforked_child_process() -> bool {
    VFORK_CONTEXT.with(|cell| {
        let ctx = cell.borrow();
        return ctx.is_some();
    })
}

// Return to parent process to continue executing
pub fn vfork_return_to_parent(
    mut context: *mut CpuContext,
    current_ref: &ThreadRef,
) -> Result<isize> {
    return restore_parent_process(context, current_ref);
}

fn restore_parent_process(mut context: *mut CpuContext, current_ref: &ThreadRef) -> Result<isize> {
    let current_thread = current!();
    let current_pid = current_ref.process().pid();

    // Restore parent file table
    let parent_file_table = {
        let mut parent_file_tables = VFORK_PARENT_FILE_TABLES.lock().unwrap();
        if let Some(table) = parent_file_tables.remove(&current_pid) {
            table
        } else {
            return_errno!(EFAULT, "couldn't restore parent file table");
        }
    };
    let mut current_file_table = current_ref.files().lock().unwrap();
    *current_file_table = parent_file_table;

    // Get child pid and restore CpuContext
    let mut child_pid = 0;
    VFORK_CONTEXT.with(|cell| {
        let mut ctx = cell.borrow_mut();
        child_pid = ctx.unwrap().0;
        unsafe { *context = ctx.unwrap().1 };
        *ctx = None;
    });

    // Set return value to child_pid
    // This will be the second time return
    Ok(child_pid as isize)
}

pub fn check_vfork_for_exec(current_ref: &ThreadRef) -> Option<(ThreadId, Option<ProcessRef>)> {
    let current_pid = current_ref.process().pid();
    if is_vforked_child_process() {
        let mut child_pid = 0;
        VFORK_CONTEXT.with(|cell| {
            let ctx = cell.borrow().unwrap();
            child_pid = ctx.0;
        });
        return Some((
            // Reuse tid which was generated when do_vfork
            ThreadId {
                tid: child_pid as u32,
            },
            // By default, use current process as parent
            None,
        ));
    } else {
        None
    }
}
