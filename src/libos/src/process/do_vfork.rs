use super::untrusted_event::{set_event, wait_event};
use super::{ProcessFilter, ProcessRef, ProcessStatus, TermStatus, ThreadId, ThreadRef};
use crate::fs::FileTable;
use crate::interrupt::broadcast_interrupts;
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

thread_local! {
    // Store the current process' vforked child and current thread's cpu context. A parent only has one vforked child at a time.
    static VFORK_CONTEXT: RefCell<Option<(pid_t, CpuContext)>> = Default::default();
}

pub fn do_vfork(mut context: *mut CpuContext) -> Result<isize> {
    let current = current!();
    trace!("vfork parent process pid = {:?}", current.process().pid());

    // Force stop all child threads
    // To prevent multiple threads do vfork simultaneously and force stop each other, the thread must change the process status at first.
    loop {
        let mut process_inner = current.process().inner();
        if process_inner.status() == ProcessStatus::Stopped {
            trace!("process is doing vfork, current thread handle force stop");
            drop(process_inner);
            handle_force_stop();
            continue;
        } else {
            trace!("current thread start vfork");
            process_inner.stop();
            break;
        }
    }

    // Stop all other child threads
    vfork_stop_all_child_thread(&current);

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

    // Save parent's file table.
    vfork_save_file_table(&current)?;

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

fn vfork_save_file_table(current: &ThreadRef) -> Result<()> {
    let parent_pid = current.process().pid();
    let mut vfork_file_tables = VFORK_PARENT_FILE_TABLES.lock().unwrap();
    let parent_file_table = {
        let mut current_file_table = current.files().lock();
        let new_file_table = current_file_table.clone();
        // FileTable contains non-cloned struct, so here we do a memory replacement to use new
        // file table in child and store the original file table in TLS.
        mem::replace(&mut *current_file_table, new_file_table)
    };

    // Insert the parent file table. The key shouldn't exist because there must be only one thread doing the vfork and save the file table for current process.
    let ret = vfork_file_tables.insert(parent_pid, parent_file_table);
    debug_assert!(ret.is_none());
    Ok(())
}

fn vfork_stop_all_child_thread(current: &ThreadRef) {
    // stop all other child threads
    loop {
        let child_threads = current.process().threads();
        let running_thread_num = child_threads
            .iter()
            .filter(|thread| !thread.is_stopped() && thread.tid() != current.tid())
            .map(|thread| {
                thread.force_stop();
                thread
            })
            .count();

        trace!("running threads num: {:?}", running_thread_num);

        if running_thread_num == 0 {
            trace!("all other threads are stopped");
            break;
        }

        // Don't hesitate. Interrupt all threads right now to stop child threads.
        broadcast_interrupts();
    }
}

// Return to parent process to continue executing
pub fn vfork_return_to_parent(
    mut context: *mut CpuContext,
    current_ref: &ThreadRef,
    child_exit_status: Option<TermStatus>, // If the child process exits, the exit status should be specified.
) -> Result<isize> {
    let child_pid = restore_parent_process(context, current_ref)?;

    if let Some(term_status) = child_exit_status {
        record_exit_child(current_ref.process().pid(), child_pid as pid_t, term_status);
    }

    // Wake parent's child thread which are all sleeping
    // Hold the process inner lock during the wake process to avoid other threads do vfork again and try to stop the thread
    let current = current!();
    let mut process_inner = current.process().inner();
    let child_threads = process_inner.threads().unwrap();
    child_threads.iter().for_each(|thread| {
        thread.resume();
        let thread_ptr = thread.raw_ptr();
        if current.raw_ptr() != thread_ptr {
            set_event(thread_ptr as *const c_void);
            info!("Thread 0x{:x} is waken", thread_ptr);
        }
    });
    process_inner.resume();

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

    // Close all child opened files
    close_files_opened_by_child(current_ref, &parent_file_table)?;

    let mut current_file_table = current_ref.files().lock();
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

fn close_files_opened_by_child(current: &ThreadRef, parent_file_table: &FileTable) -> Result<()> {
    let current_file_table = current.files().lock();
    let child_open_fds: Vec<FileDesc> = current_file_table
        .table()
        .iter()
        .enumerate()
        .filter(|(fd, _entry)| {
            // Entry is only shown in the child file table
            _entry.is_some() && parent_file_table.get_entry(*fd as FileDesc).is_err()
        })
        .map(|(fd, entry)| fd as FileDesc)
        .collect();

    drop(current_file_table);

    child_open_fds
        .iter()
        .for_each(|&fd| current.close_file(fd).expect("close child file error"));
    Ok(())
}

pub fn handle_force_stop() {
    let current = current!();
    if current.is_forced_to_stop() {
        let current_thread_ptr = current.raw_ptr();
        info!(
            "Thread 0x{:x} is forced to stop ...",
            current_thread_ptr as usize
        );

        current.inner().stop();
        while current.is_stopped() {
            wait_event(current_thread_ptr as *const c_void);
        }
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
