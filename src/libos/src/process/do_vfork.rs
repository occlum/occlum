use super::{ProcessFilter, ProcessRef, TermStatus, ThreadId, ThreadRef};
use crate::entry::context_switch::{CpuContext, CURRENT_CONTEXT};
use crate::fs::FileTable;
use crate::prelude::*;
use async_rt::wait::{Waiter, Waker};
use std::collections::HashMap;
use std::mem;

// From Man page: The calling thread is suspended until the child terminates (either normally, by calling
// _exit(2), or abnormally, after delivery of a fatal signal), or it makes a call to execve(2).
// Until that point, the child shares all memory with its parent, including the stack.
//
// Thus in this implementation, the main idea is to let child use parent's task until exit or execve.
//
// Limitation:
// 1. The child process will not have a complete process structure before execve. Thus during the time from vfork
// to new child process execve or exit, the child process just reuse the parent process's everything, including
// task, pid and etc. And also the log of child process will not start from the point that vfork returns but the
// point that execve returns.
// 2. When vfork is called and the current process has other running child threads, for Linux, the other threads remain
// running. For Occlum, this behavior is different. All the other threads will be frozen until the vfork returns or
// execve is called in the child process. The reason is that since Occlum doesn't support fork, many applications will
// use vfork to replace fork. For multi-threaded applications, if vfork doesn't stop other child threads, the application
// will be more likely to fail because the child process directly uses the VM and the file table of the parent process.

// The exit status of the child process which directly calls exit after vfork.
struct ChildExitStatus {
    pid: pid_t,
    status: TermStatus,
}

lazy_static! {
    // Store all the parents's file tables who call vfork. It will be recovered when the child exits or has its own task.
    // K: parent pid, V: parent file table
    static ref VFORK_PARENT_FILE_TABLES: SgxMutex<HashMap<pid_t, FileTable>> = SgxMutex::new(HashMap::new());
    // Store all the child process's exit status which are created with vfork and directly exit without calling execve. Because
    // these children process are only allocated with a pid, they are not managed by the usual way including exit and wait. Use
    // this special structure to record these children.
    // K: parent pid, V: exit children created with vfork.
    static ref EXIT_CHILDREN_STATUS: SgxMutex<HashMap<pid_t, Vec<ChildExitStatus>>> = SgxMutex::new(HashMap::new());
}

async_rt::task_local! {
    // Store the current process' vforked child and current thread's cpu context. A parent only has one vforked child at a time.
    static VFORK_CONTEXT: RefCell<Option<(pid_t, CpuContext)>> = Default::default();
}

pub async fn do_vfork() -> Result<isize> {
    let current = current!();
    trace!("vfork parent process pid = {:?}", current.process().pid());

    let mut curr_user_ctxt = CURRENT_CONTEXT.with(|context| context.as_ptr());

    // Generate a new pid for child process
    let child_pid = {
        let new_tid = ThreadId::new();
        new_tid.as_u32() as pid_t
    };

    // stop all other child threads
    let child_threads = current.process().threads();
    child_threads.iter().for_each(|thread| {
        if thread.tid() != current.tid() {
            thread.force_stop();
        }
    });

    // Save parent's context in TLS
    VFORK_CONTEXT.with(|cell| {
        let mut ctx = cell.borrow_mut();
        let new_context = (child_pid, unsafe { (*curr_user_ctxt).clone() });
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
    info!("vfork first return child pid = {:?}", child_pid);
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
pub async fn vfork_return_to_parent(
    mut context: *mut CpuContext,
    current_ref: &ThreadRef,
    child_exit_status: Option<TermStatus>, // If the child process exits, the exit status should be specified.
) -> Result<isize> {
    let child_pid = restore_parent_process(context, current_ref).await?;

    if let Some(term_status) = child_exit_status {
        record_exit_child(current_ref.process().pid(), child_pid as pid_t, term_status);
    }

    // Wake parent's child thread which are all sleeping
    let current = current!();
    let children = current.process().threads();
    children.iter().for_each(|thread| {
        if thread.tid() != current.tid() {
            thread.resume();
            thread.wake();
            warn!("wake children thread tid = {:?}", thread.tid());
        }
    });

    Ok(child_pid)
}

fn record_exit_child(parent_pid: pid_t, child_pid: pid_t, child_exit_status: TermStatus) {
    let child_exit_status = ChildExitStatus::new(child_pid, child_exit_status);

    let mut children_status = EXIT_CHILDREN_STATUS.lock().unwrap();
    if let Some(children) = children_status.get_mut(&parent_pid) {
        children.push(child_exit_status);
    } else {
        children_status.insert(parent_pid, vec![child_exit_status]);
    }
}

async fn restore_parent_process(
    mut context: *mut CpuContext,
    current_ref: &ThreadRef,
) -> Result<isize> {
    let current_pid = current_ref.process().pid();

    let parent_file_table = {
        let mut parent_file_tables = VFORK_PARENT_FILE_TABLES.lock().unwrap();
        if let Some(table) = parent_file_tables.remove(&current_pid) {
            table
        } else {
            return_errno!(EFAULT, "couldn't restore parent file table");
        }
    };

    // Close all child opened files
    close_files_opened_by_child(current_ref, &parent_file_table).await?;

    // Restore parent file table
    let mut current_file_table = current_ref.files().lock().unwrap();
    *current_file_table = parent_file_table;

    // Get child pid and restore CpuContext
    let mut child_pid = 0;
    VFORK_CONTEXT.with(|cell| {
        let mut ctx = cell.borrow_mut();
        child_pid = ctx.as_ref().unwrap().0;
        unsafe { *context = ctx.as_ref().unwrap().1.clone() };
        *ctx = None;
    });

    // Set return value to child_pid
    // This will be the second time return
    info!("vfork second return as parent");
    Ok(child_pid as isize)
}

pub fn check_vfork_for_exec(current_ref: &ThreadRef) -> Option<(ThreadId, Option<ProcessRef>)> {
    let current_pid = current_ref.process().pid();
    if is_vforked_child_process() {
        let mut child_pid = 0;
        VFORK_CONTEXT.with(|cell| {
            let mut _ctx = cell.borrow_mut();
            let mut ctx = _ctx.as_ref();
            child_pid = ctx.unwrap().0;
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

async fn close_files_opened_by_child(
    current: &ThreadRef,
    parent_file_table: &FileTable,
) -> Result<()> {
    let child_open_files = {
        let mut current_file_table = current.files().lock().unwrap();
        let child_open_files: Vec<FileDesc> = current_file_table
            .table()
            .iter()
            .enumerate()
            .filter_map(|(fd, entry)| {
                // Entry is only shown in the child file table
                if entry.is_some() && parent_file_table.get_entry(fd as FileDesc).is_err() {
                    Some(fd as FileDesc)
                } else {
                    None
                }
            })
            .collect();

        // Remove from file table and collect file handle for clean
        child_open_files
            .iter()
            .map(|(fd)| current_file_table.del(*fd).expect("close child file error"))
            .collect::<Vec<FileRef>>()
    };

    // Don't hold file table lock when clean for close. Otherwise, some types of the files may lock the file table when drop and will
    // cause deadlock, e.g. epoll file.
    for file in child_open_files {
        file.clean_for_close().await;
    }

    Ok(())
}

pub async fn handle_force_stop() {
    let current = current!();
    if current.is_forced_to_stop() {
        info!("Thread {} is forced to stop ...", current.tid());

        current.stop().await;
    }
}

// Wait4 unwaited child which are created with vfork and directly exit without calling execve.
pub fn wait4_exit_child_created_with_vfork(
    parent_pid: pid_t,
    child_filter: &ProcessFilter,
) -> Option<(pid_t, i32)> {
    let mut children_status = EXIT_CHILDREN_STATUS.lock().unwrap();
    if let Some(children) = children_status.get_mut(&parent_pid) {
        let unwaited_child_idx = children.iter().position(|child| match child_filter {
            ProcessFilter::WithAnyPid => true,
            ProcessFilter::WithPid(pid) => pid == child.pid(),
            ProcessFilter::WithPgid(pgid) => todo!(), // This case should be rare.
        });

        if let Some(child_idx) = unwaited_child_idx {
            let child = children.remove(child_idx);
            if children.is_empty() {
                children_status.remove(&parent_pid);
            }
            return Some((*child.pid(), child.status().as_u32() as i32));
        }
    }

    None
}

// Reap all unwaited child which are created with vfork and directly exit without calling execve.
pub fn reap_zombie_child_created_with_vfork(parent_pid: pid_t) -> Option<Vec<pid_t>> {
    let mut children_status = EXIT_CHILDREN_STATUS.lock().unwrap();

    let children = children_status.remove(&parent_pid);
    if children.is_none() {
        warn!("no vforked children found");
        return None;
    }

    Some(
        children
            .unwrap()
            .into_iter()
            .map(|child| child.pid)
            .collect(),
    )
}

impl ChildExitStatus {
    fn new(child_pid: pid_t, status: TermStatus) -> Self {
        Self {
            pid: child_pid,
            status,
        }
    }

    fn pid(&self) -> &pid_t {
        &self.pid
    }

    fn status(&self) -> &TermStatus {
        &self.status
    }
}
