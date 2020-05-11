use super::c_types::{mcontext_t, siginfo_t, ucontext_t};
use super::constants::SIGKILL;
use super::sig_stack::SigStackFlags;
use super::{SigAction, SigActionFlags, SigDefaultAction, SigSet, Signal};
use crate::prelude::*;
use crate::process::{ProcessRef, TermStatus, ThreadRef};
use crate::syscall::CpuContext;

pub fn do_rt_sigreturn(curr_user_ctxt: &mut CpuContext) -> Result<()> {
    debug!("do_rt_sigreturn");
    let last_user_ctxt = {
        let last_user_ctxt = PRE_USER_CONTEXTS.with(|ref_cell| {
            let mut stack = ref_cell.borrow_mut();
            stack.pop()
        });
        if last_user_ctxt.is_none() {
            let term_status = TermStatus::Killed(SIGKILL);
            current!().process().force_exit(term_status);
            return_errno!(
                EINVAL,
                "sigreturn should not have been called; kill this process"
            );
        }
        unsafe { &*last_user_ctxt.unwrap() }
    };
    *curr_user_ctxt = *last_user_ctxt;
    Ok(())
}

/// Deliver a queued signal for the current thread, respecting the thread's
/// signal mask.
///
/// The delivery of a signal means two things: 1) dequeuing the signal from
/// the per-thread or per-process signal queue, and 2) handling the signal
/// according to the signal disposition.
///
/// When handling a signal, one of the three actions below will be done:
///
/// 1. Ignore the signal. This is the easy part.
///
/// 2. Terminate the process if the signal is fatal. This is called "force exit".
///
/// 3. Call a user-registered signal handler. In this case, the current CPU context
/// will be modified so that the user-registered signal handler will be called
/// upon returning to the user space when the current syscall is finished.
///
/// **Requirement.** This must be called only once during the execution of a
/// syscall and at a very late stage.
///
/// **Post-condition.** The temporary signal mask of the current thread is cleared.
pub fn deliver_signal(cpu_context: &mut CpuContext) {
    let thread = current!();
    let process = thread.process();

    if process.is_forced_exit().is_none() {
        do_deliver_signal(&thread, &process, cpu_context);
    }

    // Ensure the tmp signal mask is cleared before sysret
    let mut tmp_sig_mask = thread.sig_tmp_mask().write().unwrap();
    *tmp_sig_mask = SigSet::new_empty();
}

fn do_deliver_signal(thread: &ThreadRef, process: &ProcessRef, cpu_context: &mut CpuContext) {
    loop {
        // Dequeue a signal, respecting the signal mask and tmp mask
        let sig_mask = *thread.sig_mask().read().unwrap() | *thread.sig_tmp_mask().read().unwrap();
        let signal = {
            #[rustfmt::skip]
            let signal_opt = process.sig_queues().lock().unwrap().dequeue(&sig_mask)
                .or_else(|| thread.sig_queues().lock().unwrap().dequeue(&sig_mask));
            if signal_opt.is_none() {
                return;
            }
            signal_opt.unwrap()
        };

        let continue_handling = handle_signal(signal, thread, process, cpu_context);
        if !continue_handling {
            break;
        }
    }
}

/// Force delivering the given signal to the current thread, without checking the thread's
/// signal mask.
///
/// **Post-condition.** The tmp signal mask of the current thread is all set. This avoids
/// delivering two signals during one execution of a syscall.
///
/// **Requirement.** This function can only be called at most once during the execution of
/// a syscall.
pub fn force_signal(signal: Box<dyn Signal>, cpu_context: &mut CpuContext) {
    let thread = current!();
    let process = thread.process();

    handle_signal(signal, &thread, &process, cpu_context);

    // Temporarily block all signals from being delivered until this syscall is
    // over. This ensures that the updated curr_cpu_ctxt will not be overriden
    // to deliver any other signal.
    let mut tmp_sig_mask = thread.sig_tmp_mask().write().unwrap();
    *tmp_sig_mask = SigSet::new_full();
}

fn handle_signal(
    signal: Box<dyn Signal>,
    thread: &ThreadRef,
    process: &ProcessRef,
    cpu_context: &mut CpuContext,
) -> bool {
    let is_sig_stack_full = PRE_USER_CONTEXTS.with(|ref_cell| {
        let stack = ref_cell.borrow();
        stack.full()
    });
    if is_sig_stack_full {
        panic!("the nested signal is too deep to handle");
    }

    let action = process.sig_dispositions().read().unwrap().get(signal.num());
    debug!(
        "Handle signal: signal: {:?}, action: {:?}",
        &signal, &action
    );

    let continue_handling = match action {
        SigAction::Ign => true,
        SigAction::Dfl => {
            let default_action = SigDefaultAction::from_signum(signal.num());
            match default_action {
                SigDefaultAction::Ign => true,
                SigDefaultAction::Term | SigDefaultAction::Core => {
                    let term_status = TermStatus::Killed(signal.num());
                    process.force_exit(term_status);
                    false
                }
                SigDefaultAction::Stop => {
                    warn!("SIGSTOP is unsupported");
                    true
                }
                SigDefaultAction::Cont => {
                    warn!("SIGCONT is unsupported");
                    true
                }
            }
        }
        SigAction::User {
            handler_addr,
            flags,
            restorer_addr,
            mask,
        } => {
            let ret = handle_signals_by_user(
                signal,
                handler_addr,
                flags,
                restorer_addr,
                mask,
                cpu_context,
            );
            if let Err(_) = ret {
                todo!("kill the process if any error");
            }
            false
        }
    };
    continue_handling
}

fn handle_signals_by_user(
    signal: Box<dyn Signal>,
    handler_addr: usize,
    flags: SigActionFlags,
    restorer_addr: usize,
    mask: SigSet,
    curr_user_ctxt: &mut CpuContext,
) -> Result<()> {
    // Represent the user stack in a memory safe way
    let mut user_stack = {
        let get_stack_top = || -> usize {
            if flags.contains(SigActionFlags::SA_ONSTACK) {
                let thread = current!();
                let sig_stack = thread.sig_stack().lock().unwrap();
                if let Some(stack) = *sig_stack {
                    if !stack.contains(curr_user_ctxt.rsp as usize) {
                        let stack_top = stack.sp() + stack.size();
                        return stack_top;
                    }
                }
            }
            const BIG_ENOUGH_GAP: u64 = 1024;
            let stack_top = (curr_user_ctxt.rsp - BIG_ENOUGH_GAP) as usize;
            stack_top
        };
        let stack_top = get_stack_top();
        let stack_size = {
            const BIG_ENOUGH_SIZE: u64 = 4096;
            BIG_ENOUGH_SIZE as usize
        };
        // TODO: validate the memory range of the stack
        unsafe { Stack::new(stack_top, stack_size)? }
    };

    // Prepare the user stack in four steps.
    //
    // 1. Allocate and init siginfo_t on the user stack.
    let info = {
        let info = user_stack.alloc::<siginfo_t>()?;
        *info = signal.to_info();
        info as *mut siginfo_t
    };
    // 2. Allocate and init ucontext_t on the user stack.
    let ucontext = {
        // The x86 calling convention requires rsp to be 16-byte aligned.
        // The following allocation on stack is right before we "call" the
        // signal handler. So we need to make sure the allocation is at least
        // 16-byte aligned.
        let ucontext = user_stack.alloc_aligned::<ucontext_t>(16)?;
        // TODO: set all fields in ucontext
        *ucontext = unsafe { std::mem::zeroed() };
        ucontext as *mut ucontext_t
    };
    // 3. Save the current user CPU context on the stack of the signal handler
    // so that we can restore the CPU context upon `sigreturn` syscall.
    let saved_user_ctxt = {
        let saved_user_ctxt = unsafe { &mut (*ucontext).uc_mcontext.inner };
        *saved_user_ctxt = *curr_user_ctxt;
        saved_user_ctxt as *mut CpuContext
    };
    // 4. Set up the call return address on the stack before we "call" the signal handler
    let handler_stack_top = {
        let handler_stack_top = user_stack.alloc::<usize>()?;
        *handler_stack_top = restorer_addr;
        handler_stack_top as *mut usize
    };
    // TODO: mask signals while the signal handler is executing

    // Modify the current user CPU context so that the signal handler will
    // be "called" upon returning back to the user space and when the signal
    // handler finishes, the CPU will jump to the restorer.
    curr_user_ctxt.rsp = handler_stack_top as u64;
    curr_user_ctxt.rip = handler_addr as u64;
    // Prepare the three arguments for the signal handler
    curr_user_ctxt.rdi = signal.num().as_u8() as u64;
    curr_user_ctxt.rsi = info as u64;
    curr_user_ctxt.rdx = ucontext as u64;

    PRE_USER_CONTEXTS.with(|ref_cell| {
        let mut stack = ref_cell.borrow_mut();
        stack.push(saved_user_ctxt).unwrap();
    });
    Ok(())
}

/// Represent and manipulate a stack in a memory-safe way
struct Stack {
    pointer: usize,
    bottom: usize,
}

impl Stack {
    /// Create a new region of memory to use as stack
    pub unsafe fn new(stack_top: usize, stack_size: usize) -> Result<Stack> {
        if stack_top <= stack_size {
            return_errno!(EINVAL, "stack address may underflow");
        }
        let pointer = stack_top;
        let bottom = stack_top - stack_size;
        Ok(Stack { pointer, bottom })
    }

    /// Get the size of the free space in the stack
    pub fn size(&self) -> usize {
        self.pointer - self.bottom
    }

    /// Allocate a mutable object on the stack.
    ///
    /// The alignment of the object will be `std::mem::size_of::<T>()`.
    pub fn alloc<T>(&mut self) -> Result<&mut T> {
        self.do_alloc_aligned::<T>(1)
    }

    /// Allocate a mutable object on the stack.
    ///
    /// The alignment of the object will be `max(align, std::mem::size_of::<T>())`.
    pub fn alloc_aligned<T>(&mut self, align: usize) -> Result<&mut T> {
        if !align.is_power_of_two() {
            return_errno!(EINVAL, "align must be a power of two");
        }
        self.do_alloc_aligned::<T>(align)
    }

    /// Allocate a mutable object on the stack.
    ///
    /// The alignment of the object will be `max(align, std::mem::size_of::<T>())`.
    fn do_alloc_aligned<T>(&mut self, align: usize) -> Result<&mut T> {
        // Check precondition
        debug_assert!(align.is_power_of_two());

        // Calculate the pointer of the object
        let new_pointer = {
            let size = std::mem::size_of::<T>();
            let align = std::mem::align_of::<T>().max(align);

            let mut pointer = self.pointer;
            if pointer < size {
                return_errno!(ENOMEM, "not enough memory");
            }
            pointer -= size;
            pointer = align_down(pointer, align);
            if pointer < self.bottom {
                return_errno!(ENOMEM, "not enough memory");
            }
            pointer
        };
        self.pointer = new_pointer;

        let obj_ref = unsafe { &mut *(new_pointer as *mut T) };
        Ok(obj_ref)
    }
}

thread_local! {
    static PRE_USER_CONTEXTS: RefCell<CpuContextStack> = Default::default();
}

#[derive(Debug, Default)]
struct CpuContextStack {
    stack: [Option<*mut CpuContext>; 32],
    count: usize,
}

impl CpuContextStack {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn full(&self) -> bool {
        self.count == self.stack.len()
    }

    pub fn empty(&self) -> bool {
        self.count == 0
    }

    pub fn push(&mut self, cpu_context: *mut CpuContext) -> Result<()> {
        if self.full() {
            return_errno!(ENOMEM, "cpu context stack is full");
        }
        self.stack[self.count] = Some(cpu_context);
        self.count += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<*mut CpuContext> {
        if self.empty() {
            return None;
        }
        self.count -= 1;
        self.stack[self.count].take()
    }
}
