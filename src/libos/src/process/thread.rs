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
    stack_addr: usize,
    ptid: Option<*mut pid_t>,
    ctid: Option<*mut pid_t>,
    new_tls: Option<usize>,
) -> Result<pid_t, Error> {
    info!("clone: flags: {:?}, stack_addr: {:?}, ptid: {:?}, ctid: {:?}, new_tls: {:?}",
          flags, stack_addr, ptid, ctid, new_tls);
    // TODO: return error for unsupported flags

    let current_ref = get_current();
    let current = current_ref.lock().unwrap();

    let (new_thread_pid, new_thread_ref) = {
        let task = new_thread_task(stack_addr, new_tls)?;
        let vm_ref = current.get_vm().clone();
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
        unsafe { *ptid = new_thread_pid; }
    }

    task::enqueue_task(new_thread_ref);
    Ok(new_thread_pid)
}

fn new_thread_task(user_stack: usize, new_tls: Option<usize>) -> Result<Task, Error> {
    // The calling convention of Occlum clone syscall requires the user to
    // restore the entry point of the new thread at the top of the user stack.
    let user_entry = unsafe {
        *(user_stack as *mut usize)
        // TODO: check user_entry is a cfi_label
    };
    Ok(Task {
        user_stack_addr: user_stack,
        user_entry_addr: user_entry,
        // TODO: use 0 as the default value is not safe
        user_fsbase_addr: new_tls.unwrap_or(0),
        ..Default::default()
    })
}

pub fn do_set_tid_address(tidptr: *mut pid_t) -> Result<pid_t, Error> {
    info!("set_tid_address: tidptr: {:#x}", tidptr as usize);
    let current_ref = get_current();
    let mut current = current_ref.lock().unwrap();
    current.clear_child_tid = Some(tidptr);
    Ok(current.get_tid())
}
