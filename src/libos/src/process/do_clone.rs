use std::ptr::NonNull;

use super::table::{self};
use super::thread::{Thread, ThreadBuilder};
use crate::entry::context_switch::{CpuContext, GpRegs};
use crate::prelude::*;
use crate::vm::{ProcessVM, VMRange};

/// Create and execute a new thread.
pub async fn do_clone(
    flags: CloneFlags,
    user_rsp: usize,
    ptid: Option<NonNull<pid_t>>,
    ctid: Option<NonNull<pid_t>>,
    new_tls: Option<usize>,
) -> Result<pid_t> {
    debug!(
        "clone: flags: {:?}, stack_addr: {:?}, ptid: {:?}, ctid: {:?}, new_tls: {:?}",
        flags, user_rsp, ptid, ctid, new_tls
    );

    check_clone_args(flags, user_rsp, ptid, ctid, new_tls)?;

    // Get thread entry, an implicit argument passed on the stack.
    //
    // The calling convention of Occlum clone syscall requires the user to
    // store the entry point of the new thread at the top of the user stack.
    //
    // FIXME: this is workaround to passing more than 6 arguments in syscall.
    // TODO: add pointer checking
    let thread_entry = unsafe { *(user_rsp as *mut usize) };

    let fs_base = if let Some(tls) = new_tls {
        tls
    } else {
        // Set the default user fsbase to an address on user stack, which is
        // a relatively safe address in case the user program uses %fs before
        // initializing fs base address.
        guess_user_stack_bound(current!().vm(), user_rsp)
            .await?
            .start()
    };
    let init_cpu_state = CpuContext {
        gp_regs: GpRegs {
            rsp: user_rsp as _,
            rip: thread_entry as _,
            ..Default::default()
        },
        fs_base: fs_base as _,
        ..Default::default()
    };

    let new_thread_ref = {
        let current = current!();
        let vm = current.vm().clone();
        let files = current.files().clone();
        let nice = current.nice().clone();
        let rlimits = current.rlimits().clone();
        let fs = current.fs().clone();
        let name = current.name().clone();
        let sig_mask = current.sig_mask();

        let mut builder = ThreadBuilder::new()
            .process(current.process().clone())
            .vm(vm)
            .fs(fs)
            .files(files)
            .name(name)
            .nice(nice)
            .rlimits(rlimits)
            .sig_mask(sig_mask);
        if let Some(ctid) = ctid {
            builder = builder.clear_ctid(ctid);
        }
        builder.build()?
    };
    trace!("new thread sigmask = {:?}", new_thread_ref.sig_mask());
    let new_tid = new_thread_ref.tid();
    let process = new_thread_ref.process();
    // If the current thread is forced to exit, there is no need to let the new thread to execute.
    if process.is_forced_to_exit() {
        new_thread_ref.exit_early(process.term_status().unwrap());
        return Ok(0);
    }

    table::add_thread(new_thread_ref.clone());
    info!("Thread created: tid = {}", new_tid);

    if flags.contains(CloneFlags::CLONE_PARENT_SETTID) {
        debug_assert!(ptid.is_some());
        unsafe {
            *ptid.unwrap().as_ptr() = new_tid;
        }
    }
    if flags.contains(CloneFlags::CLONE_CHILD_SETTID) {
        debug_assert!(ctid.is_some());
        unsafe {
            *ctid.unwrap().as_ptr() = new_tid;
        }
    }

    async_rt::task::spawn(crate::entry::thread::main_loop(
        new_thread_ref,
        init_cpu_state,
    ));
    Ok(new_tid)
}

/// Clone flags.
bitflags! {
    pub struct CloneFlags : u32 {
        const CLONE_VM              = 0x00000100;
        const CLONE_FS              = 0x00000200;
        const CLONE_FILES           = 0x00000400;
        const CLONE_SIGHAND         = 0x00000800;
        const CLONE_PIDFD           = 0x00001000;
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

fn check_clone_args(
    flags: CloneFlags,
    user_rsp: usize,
    ptid: Option<NonNull<pid_t>>,
    ctid: Option<NonNull<pid_t>>,
    new_tls: Option<usize>,
) -> Result<()> {
    check_clone_flags(flags)?;

    let need_ptid = flags.contains(CloneFlags::CLONE_PARENT_SETTID);
    if need_ptid != ptid.is_some() {
        return_errno!(EINVAL, "ptid is not consistent with flags");
    }

    let need_ctid = flags.contains(CloneFlags::CLONE_CHILD_SETTID)
        || flags.contains(CloneFlags::CLONE_CHILD_CLEARTID);
    if need_ctid != ctid.is_some() {
        return_errno!(EINVAL, "ctid is not consistent with flags");
    }

    Ok(())
}

/// Check whether clone flags are valid.
///
/// The current implementation of clone, which is much less general than the one in Linux,
/// essentially supports creating threads only. So the valid combinations of clone flags
/// are quite limited.
///
/// # Mandatory flags
///
/// The following flags must be given. If not given, errors will be reported:
/// ```
/// CLONE_VM
/// CLONE_THREAD
/// CLONE_SIGHAND
/// CLONE_FILES
/// CLONE_FS
/// CLONE_SETTLS
/// CLONE_SIGHAND
/// CLONE_SYSVSEM
/// ```
///
/// # Optional flags
///
/// The following flags can be given and are supported:
/// ```
/// CLONE_CHILD_CLEARTID
/// CLONE_CHILD_SETTID
/// CLONE_PARENT_SETTID
/// ```
///
/// # Ignored flags
///
/// The following flags are ignored silently:
/// ```
/// CLONE_DETACHED
/// CLONE_IO
/// CLONE_PARENT
/// ```
///
/// # Unsupported flags
///
/// The following flags are unsupported; giving these flags triggers errors.
/// ```
/// CLONE_VFORK
/// CLONE_NEWCGROUP
/// CLONE_NEWIPC
/// CLONE_NEWNET
/// CLONE_NEWNS
/// CLONE_NEWPID
/// CLONE_NEWUSER
/// CLONE_NEWUTS
/// CLONE_PIDFD
/// CLONE_PTRACE
/// CLONE_UNTRACED
/// ```
fn check_clone_flags(flags: CloneFlags) -> Result<()> {
    lazy_static! {
        static ref MANDATORY_FLAGS: CloneFlags = {
            CloneFlags::CLONE_VM
                | CloneFlags::CLONE_THREAD
                | CloneFlags::CLONE_SIGHAND
                | CloneFlags::CLONE_FILES
                | CloneFlags::CLONE_FS
                | CloneFlags::CLONE_SETTLS
                | CloneFlags::CLONE_SIGHAND
                | CloneFlags::CLONE_SYSVSEM
        };
        static ref UNSUPPORTED_FLAGS: CloneFlags = {
            CloneFlags::CLONE_VFORK
                | CloneFlags::CLONE_NEWCGROUP
                | CloneFlags::CLONE_NEWIPC
                | CloneFlags::CLONE_NEWNET
                | CloneFlags::CLONE_NEWNS
                | CloneFlags::CLONE_NEWPID
                | CloneFlags::CLONE_NEWUSER
                | CloneFlags::CLONE_NEWUTS
                | CloneFlags::CLONE_PIDFD
                | CloneFlags::CLONE_PTRACE
                | CloneFlags::CLONE_UNTRACED
        };
    }

    if !flags.contains(*MANDATORY_FLAGS) {
        return_errno!(EINVAL, "missing mandatory flags");
    }
    if flags.contains(*UNSUPPORTED_FLAGS) {
        return_errno!(EINVAL, "found unsupported flags");
    }

    Ok(())
}

async fn guess_user_stack_bound(vm: &ProcessVM, user_rsp: usize) -> Result<VMRange> {
    // The first case is most likely
    if let Ok(stack_range) = vm.find_mmap_region(user_rsp).await {
        Ok(stack_range)
    }
    // The next three cases are very unlikely, but valid
    else if vm.get_stack_range().contains(user_rsp) {
        Ok(*vm.get_stack_range())
    } else if vm.get_heap_range().contains(user_rsp) {
        Ok(*vm.get_heap_range())
    }
    // Invalid
    else {
        return_errno!(ESRCH, "invalid rsp")
    }
}
