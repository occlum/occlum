use super::vm::VMRange;
use super::*;

pub struct ThreadGroup {
    threads: Vec<ProcessRef>,
}

bitflags! {
    pub struct CloneFlags : u32 {
        const CLONE_VM              = 0x00000100;
        const CLONE_FS              = 0x00000200;
        const CLONE_FILES           = 0x00000400;
        const CLONE_SIGHAND         = 0x00000800;
        const CLONE_PTRACE          = 0x00002000;
        const CLONE_VFORK           = 0x00004000;
        const CLONE_PARENT          = 0x00008000;
        const CLONE_THREAD          = 0x00010000;
        const CLONE_NEWNS           = 0x00020000;
        const CLONE_SYSVSEM         = 0x00040000;
        const CLONE_SETTLS          = 0x00080000;
        const CLONE_PARENT_SETTID   = 0x00100000;
        const CLONE_CHILD_CLEARTID  = 0x00200000;
        const CLONE_DETACHED        = 0x00400000;
        const CLONE_UNTRACED        = 0x00800000;
        const CLONE_CHILD_SETTID    = 0x01000000;
        const CLONE_NEWCGROUP       = 0x02000000;
        const CLONE_NEWUTS          = 0x04000000;
        const CLONE_NEWIPC          = 0x08000000;
        const CLONE_NEWUSER         = 0x10000000;
        const CLONE_NEWPID          = 0x20000000;
        const CLONE_NEWNET          = 0x40000000;
        const CLONE_IO              = 0x80000000;
    }
}

pub fn do_clone(
    flags: CloneFlags,
    user_rsp: usize,
    ptid: Option<*mut pid_t>,
    ctid: Option<*mut pid_t>,
    new_tls: Option<usize>,
) -> Result<pid_t> {
    info!(
        "clone: flags: {:?}, stack_addr: {:?}, ptid: {:?}, ctid: {:?}, new_tls: {:?}",
        flags, user_rsp, ptid, ctid, new_tls
    );
    // TODO: return error for unsupported flags

    let current_ref = get_current();
    let current = current_ref.lock().unwrap();

    // The calling convention of Occlum clone syscall requires the user to
    // store the entry point of the new thread at the top of the user stack.
    let thread_entry = unsafe {
        *(user_rsp as *mut usize)
        // TODO: check user_entry is a cfi_label
    };

    let (new_thread_pid, new_thread_ref) = {
        let vm_ref = current.get_vm().clone();
        let task = {
            let vm = vm_ref.lock().unwrap();
            let user_stack_range = guess_user_stack_bound(&vm, user_rsp)?;
            let user_stack_base = user_stack_range.end();
            let user_stack_limit = user_stack_range.start();
            unsafe {
                Task::new(
                    thread_entry,
                    user_rsp,
                    user_stack_base,
                    user_stack_limit,
                    new_tls,
                )?
            }
        };
        let files_ref = current.get_files().clone();
        let rlimits_ref = current.get_rlimits().clone();
        let cwd = &current.cwd;
        Process::new(cwd, task, vm_ref, files_ref, rlimits_ref)?
    };

    if let Some(ctid) = ctid {
        let mut new_thread = new_thread_ref.lock().unwrap();
        new_thread.clear_child_tid = Some(ctid);
    }

    // TODO: always get parent lock first to avoid deadlock
    {
        let parent_ref = current.parent.as_ref().unwrap();
        let mut parent = parent_ref.lock().unwrap();
        let mut new_thread = new_thread_ref.lock().unwrap();
        parent.children.push(Arc::downgrade(&new_thread_ref));
        new_thread.parent = Some(parent_ref.clone());

        new_thread.tgid = current.tgid;
    }

    process_table::put(new_thread_pid, new_thread_ref.clone());

    if let Some(ptid) = ptid {
        unsafe {
            *ptid = new_thread_pid;
        }
    }

    task::enqueue_task(new_thread_ref);
    Ok(new_thread_pid)
}

pub fn do_set_tid_address(tidptr: *mut pid_t) -> Result<pid_t> {
    info!("set_tid_address: tidptr: {:#x}", tidptr as usize);
    let current_ref = get_current();
    let mut current = current_ref.lock().unwrap();
    current.clear_child_tid = Some(tidptr);
    Ok(current.get_tid())
}

fn guess_user_stack_bound(vm: &ProcessVM, user_rsp: usize) -> Result<&VMRange> {
    // The first case is most likely
    if let Ok(stack_range) = vm.find_mmap_region(user_rsp) {
        Ok(stack_range)
    }
    // The next three cases are very unlikely, but valid
    else if vm.get_stack_range().contains(user_rsp) {
        Ok(vm.get_stack_range())
    } else if vm.get_heap_range().contains(user_rsp) {
        Ok(vm.get_heap_range())
    }
    // Invalid
    else {
        return_errno!(ESRCH, "invalid rsp")
    }
}
