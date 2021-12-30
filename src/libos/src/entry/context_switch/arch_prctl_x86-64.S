# In SGX simulation mode, we don't use wrfsbase directly. Instead, we use arch_prctl syscall.

#if SGX_MODE_SIM

#define __ASSEMBLY__
#include "switch.h"

    .file "arch_prctl.S"

    .global __occlum_arch_prctl
    .type __occlum_arch_prctl, @function
__occlum_arch_prctl:
    // A system-call is done via the syscall instruction.
    // This clobbers RCX and R11 as well as the RAX return value, but other registers are preserved.
    // The number of the syscall has to be passed in register RAX.
    mov $ARCH_PRCTL, %eax
    syscall

    // Register RAX contains the result of the system-call.
    cmp $0, %rax
    jne __syscall_error
    ret

__syscall_error: // This should never happen
    ud2

#endif // SGX_MODE_SIM