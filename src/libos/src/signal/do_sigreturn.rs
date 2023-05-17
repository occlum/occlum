use aligned::{Aligned, A16};
use core::arch::x86_64::{_fxrstor, _fxsave};
use core::convert::TryInto;
use std::{ptr, slice};

use super::c_types::{mcontext_t, siginfo_t, ucontext_t};
use super::constants::SIGKILL;
use super::sig_queues::dequeue_signal;
use super::sig_stack::SigStackFlags;
use super::{SigAction, SigActionFlags, SigDefaultAction, SigSet, Signal};
use crate::entry::context_switch::{CpuContext, FpRegs, CURRENT_CONTEXT};
use crate::lazy_static::__Deref;
use crate::prelude::*;
use crate::process::{ProcessRef, TermStatus, ThreadRef};
use crate::vm::{VMRange, PAGE_SIZE};

pub fn do_rt_sigreturn() -> Result<()> {
    debug!("do_rt_sigreturn");
    let last_ucontext = {
        let last_ucontext = SIGNAL_CONTEXT.with(|signal_context| signal_context.borrow_mut().pop());

        // Handle a (very unlikely) error condition
        if last_ucontext.is_none() {
            let term_status = TermStatus::Killed(SIGKILL);
            current!().process().force_exit(term_status);
            return_errno!(
                EINVAL,
                "sigreturn should not have been called; kill this process"
            );
        }

        unsafe { &*last_ucontext.unwrap() }
    };

    // Restore sigmask
    current!().set_sig_mask(SigSet::from_c(last_ucontext.uc_sigmask));
    // Restore user context
    CURRENT_CONTEXT.with(|_context| {
        let mut context = _context.borrow_mut();
        context.gp_regs = last_ucontext.uc_mcontext.gp_regs;
        unsafe {
            context.fp_regs.save_from_slice(&last_ucontext.fpregs);
        }
    });
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
///
/// **Interaction with force_signal.** If force_signal is called during a syscall,
/// then deliver_signal won't deliver any signals.
pub async fn deliver_signal() {
    let thread = current!();

    if thread.process().is_forced_to_exit() || thread.is_forced_to_stop() {
        return;
    }

    if !forced_signal_flag::get() {
        do_deliver_signal(&thread).await;
    } else {
        forced_signal_flag::reset();
    }
}

async fn do_deliver_signal(thread: &ThreadRef) {
    loop {
        let sig_mask = thread.sig_mask();
        let signal = match dequeue_signal(thread, sig_mask) {
            Some(signal) => signal,
            None => return,
        };

        let continue_handling = handle_signal(signal, thread).await;
        if !continue_handling {
            return;
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
pub async fn force_signal(signal: Box<dyn Signal>) {
    let thread = current!();

    assert!(forced_signal_flag::get() == false);
    forced_signal_flag::set();

    handle_signal(signal, &thread).await;
}

async fn handle_signal(signal: Box<dyn Signal>, thread: &ThreadRef) -> bool {
    let action = thread
        .process()
        .sig_dispositions()
        .read()
        .unwrap()
        .get(signal.num());
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
                    thread.process().force_exit(term_status);
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
            let mut context = CURRENT_CONTEXT.with(|_context| {
                let mut context = _context.borrow_mut();
                context
            });
            let curr_user_ctxt = context.deref_mut();
            let ret = handle_signals_by_user(
                signal,
                thread,
                handler_addr,
                flags,
                restorer_addr,
                mask,
                curr_user_ctxt,
            )
            .await;
            if let Err(_) = ret {
                todo!("kill the process if any error");
            }
            false
        }
    };
    continue_handling
}

async fn handle_signals_by_user<'a>(
    signal: Box<dyn Signal>,
    thread: &'a ThreadRef,
    handler_addr: usize,
    flags: SigActionFlags,
    restorer_addr: usize,
    new_sig_mask: SigSet,
    curr_user_ctxt: &'a mut CpuContext,
) -> Result<()> {
    // Set a new signal mask and get the old one
    let new_sig_mask = if flags.contains(SigActionFlags::SA_NODEFER) {
        new_sig_mask
    } else {
        // Block the current signal while executing the signal handler
        new_sig_mask + signal.num()
    };
    let old_sig_mask = thread.set_sig_mask(new_sig_mask);

    // Represent the user stack in a memory safe way
    let mut user_stack = {
        let get_stack_top = || -> usize {
            if flags.contains(SigActionFlags::SA_ONSTACK) {
                let thread = current!();
                let sig_stack = thread.sig_stack().lock().unwrap();
                if let Some(stack) = *sig_stack {
                    if !stack.contains(curr_user_ctxt.gp_regs.rsp as usize) {
                        let stack_top = stack.sp() + stack.size();
                        return stack_top;
                    }
                }
            }

            // The 128-byte area beyond the location pointed to by %rsp is considered to be reserved and
            // shall not be modified by signal or interrupt handlers. Therefore, functions may use this
            // area for temporary data that is not needed across function calls. In particular, leaf functions
            // may use this area for their entire stack frame, rather than adjusting the stack pointer in the
            // prologue and epilogue. This area is known as the red zone.
            const RED_ZONE: u64 = 128;
            let stack_top = (curr_user_ctxt.gp_regs.rsp - RED_ZONE).try_into().unwrap();
            stack_top
        };

        let stack_top = get_stack_top();
        let stack_size = {
            // define MINSIGSTKSZ with the same value in libc
            const MINSIGSTKSZ: u64 = 2048;

            // Use MINSIGSTKSZ as the default stack size
            // When the stack is not enough, the #PF happens in user's code
            MINSIGSTKSZ as usize
        };

        // Validate the stack
        let stack_range = VMRange::new(
            align_down(stack_top - stack_size, PAGE_SIZE),
            align_up(stack_top, PAGE_SIZE),
        )
        .unwrap();

        let mem_chunks = thread.vm().mem_chunks().read().await;
        if mem_chunks
            .iter()
            .find(|chunk| chunk.range().is_superset_of(&stack_range))
            .is_none()
        {
            return_errno!(ENOMEM, "the user stack is not enough to handle the signal");
        }

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
        // Save the old sigmask
        ucontext.uc_sigmask = old_sig_mask.to_c();
        // Save the user context
        ucontext.uc_mcontext.gp_regs = curr_user_ctxt.gp_regs;
        // Save the floating point registers
        let fp_regs = &mut curr_user_ctxt.fp_regs;
        if !fp_regs.is_valid() {
            // We need a valid fxsave structure in the buffer,
            // because the app may modify part of it to update the
            // floating point after the signal handler finished.
            fp_regs.save();
        }
        ucontext.fpregs.copy_from_slice(fp_regs.as_slice());

        ucontext as *mut ucontext_t
    };
    // 3. Set up the call return address on the stack before we "call" the signal handler
    let handler_stack_top = {
        let handler_stack_top = user_stack.alloc::<usize>()?;
        *handler_stack_top = restorer_addr;
        handler_stack_top as *mut usize
    };

    // Modify the current user CPU context so that the signal handler will
    // be "called" upon returning back to the user space and when the signal
    // handler finishes, the CPU will jump to the restorer.
    {
        let gp_regs = &mut curr_user_ctxt.gp_regs;
        gp_regs.rsp = handler_stack_top as u64;
        gp_regs.rip = handler_addr as u64;
        // Prepare the three arguments for the signal handler
        gp_regs.rdi = signal.num().as_u8() as u64;
        gp_regs.rsi = info as u64;
        gp_regs.rdx = ucontext as u64;

        let fp_regs = &mut curr_user_ctxt.fp_regs;
        fp_regs.clear();
    }

    SIGNAL_CONTEXT.with(|signal_context| unsafe { signal_context.borrow_mut().push(ucontext) });
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

async_rt::task_local! {
    static SIGNAL_CONTEXT: RefCell<SignalContext> = RefCell::new(SignalContext::new());
}

struct SignalContext {
    context: *mut ucontext_t,
}

// Using using userlevel memory to cache the old context
// Reference https://man7.org/linux/man-pages/man2/getcontext.2.html
// "Here uc_link points to the context that will be resumed when the current context terminates."
impl SignalContext {
    pub fn new() -> Self {
        SignalContext {
            context: ptr::null_mut(),
        }
    }

    pub unsafe fn push(&mut self, cpu_context: *mut ucontext_t) {
        if cpu_context.is_null() {
            panic!("the signal context is null");
        }

        unsafe {
            (*cpu_context).uc_link = self.context;
        }
        self.context = cpu_context;
    }

    pub fn pop(&mut self) -> Option<*mut ucontext_t> {
        if self.context.is_null() {
            return None;
        }

        let cpu_context = self.context;
        unsafe {
            self.context = (*cpu_context).uc_link;
            Some(cpu_context)
        }
    }
}

unsafe impl Send for SignalContext {}

// This module maintain a flag about whether a task already has a forced signal.
// The goal is to ensure that during the execution of a syscall at most one
// signal is forced.
mod forced_signal_flag {
    use core::cell::Cell;

    pub fn get() -> bool {
        HAS_FORCED_SIGNAL.with(|has_forced_signal| has_forced_signal.get())
    }

    pub fn set() {
        HAS_FORCED_SIGNAL.with(|has_forced_signal| {
            has_forced_signal.set(true);
        })
    }

    pub fn reset() {
        HAS_FORCED_SIGNAL.with(|has_forced_signal| {
            has_forced_signal.set(false);
        })
    }

    task_local! {
        static HAS_FORCED_SIGNAL: Cell<bool> = Cell::new(false);
    }
}
