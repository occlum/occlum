# In SGX simulation mode, we don't use wrfsbase directly. Instead, we use arch_prctl syscall.

#if SGX_MODE_SIM

#define __ASSEMBLY__
#include "task.h"

    .file "arch_prctl.S"

    .global __arch_prctl
    .type __arch_prctl, @function
__arch_prctl:
    // A system-call is done via the syscall instruction.
    // This clobbers RCX and R11 as well as the RAX return value, but other registers are preserved.
    // The number of the syscall has to be passed in register RAX.
    pushq %rcx
    pushq %r11

    mov $ARCH_PRCTL, %eax
    syscall

    // Register RAX contains the result of the system-call.
    cmp $0, %rax
    jne __syscall_error
    popq %r11
    popq %rcx
    ret

__syscall_error: // This should never happen
    ud2

#endif // SGX_MODE_SIM