#define __ASSEMBLY__
#include "task.h"


    .file "syscall_entry_x86-64.S"
    .global __occlum_syscall_linux_abi
    .type __occlum_syscall_linux_abi, @function
__occlum_syscall_linux_abi:
    //      num  - %rax
    //      arg0 - %rdi
    //      arg1 - %rsi
    //      arg2 - %rdx
    //      arg3 - %r10
    //      arg4 - %r8
    //      arg5 - *r9
    //      return address - *(%rsp)

    push %rbp
    movq %rsp, %rbp
    // The return address is now in 8(%rbp).
    // The original %rbp is now in (%rbp).
    // The original %rsp is now in %rbp + 8.

    // Save the target CPU state when `call __occlum_syscall` is returned in
    // a CpuContext struct. The registers are saved in the reverse order of 
    // the fields in CpuContext.
    pushfq
    push 8(%rbp)    // save %rip
    push %rbp       // save %rsp, but not the final value, to be adjusted later
    push %rcx
    push %rax
    push %rdx
    push %rbx
    push (%rbp)     // save %rbp
    push %rsi
    push %rdi
    push %r15
    push %r14
    push %r13
    push %r12
    push %r11
    push %r10
    push %r9
    push %r8
    // Make %rdi points to CpuContext.
    mov %rsp, %rdi
    // The target %rsp is actuall the saved one plus 16
    addq $16, (15*8)(%rdi)

    // Get current task
    movq %gs:(TD_TASK_OFFSET), %r12
    // Switch to the kernel stack
    movq TASK_KERNEL_RSP(%r12), %rsp
    // Switch to the kernel TLS by setting fsbase. Different implementation for HW and SIM modes.
#if SGX_MODE_SIM
    pushq %rdi
    pushq %rsi

    movq $ARCH_SET_FS, %rdi
    movq TASK_KERNEL_FS(%r12), %rsi
    call __arch_prctl

    popq %rsi
    popq %rdi
#else // SGX_MODE_HW
    movq TASK_KERNEL_FS(%r12), %r11
    wrfsbase %r11
#endif  
    // Switch to kernel stack base and limit
    movq TASK_KERNEL_STACK_BASE(%r12), %r11
    movq %r11, %gs:TD_STACK_BASE
    movq TASK_KERNEL_STACK_LIMIT(%r12), %r11
    movq %r11, %gs:TD_STACK_LIMIT

    call occlum_syscall

    // This should never happen!
    ud2


    .global __occlum_sysret
    .type __occlum_sysret, @function
__occlum_sysret:
    // Arguments:
    //      %rdi - user_context: &mut CpuContext

    // Jumping back to the user space itself is easy, but not so easy when 
    // we need to set all other registers to some specified values. To overcome 
    // this difficulty, the most obvious choice is using a ret instruction, which
    // can set %rip and %rsp at the same time. So we must set -8(%rsp) to the 
    // value of the target %rip before ret, where %rsp has the value of target
    // %rsp.
    //
    // But there is a catch: it is dangerous to modify the value at -8(%rsp), 
    // which may still be used by the user space (remember red zone and 
    // signal handler?). So we need to use a stack location outside the 
    // 128-byte red zone. So in this function, we store the target %rip value
    // in $-136(%rsp) and do `ret 128` at the end of this function.
    subq $136, (15*8)(%rdi)
    movq (15*8)(%rdi), %r11
    movq (16*8)(%rdi), %r12
    movq %r12, (%r11)

    // Get current task
    movq %gs:(TD_TASK_OFFSET), %r12
    // Switch to the user TLS. Different implementation for HW and SIM modes.
#if SGX_MODE_SIM
    pushq %rdi

    movq $ARCH_SET_FS, %rdi
    movq TASK_USER_FS(%r12), %rsi
    call __arch_prctl

    popq %rdi
#else // SGX_MODE_HW
    movq TASK_USER_FS(%r12), %r11
    wrfsbase %r11
#endif
    // Switch to user stack base and limit
    movq TASK_USER_STACK_BASE(%r12), %r11
    movq %r11, %gs:TD_STACK_BASE
    movq TASK_USER_STACK_LIMIT(%r12), %r11
    movq %r11, %gs:TD_STACK_LIMIT

    // Restore flags first
    leaq (17*8)(%rdi), %rsp
    popfq

    // Make %rsp points to the CPU context
    mov %rdi, %rsp
    // Restore the CPU context of the user space
    pop %r8
    pop %r9
    pop %r10
    pop %r11
    pop %r12
    pop %r13
    pop %r14
    pop %r15
    pop %rdi
    pop %rsi
    pop %rbp
    pop %rbx
    pop %rdx
    pop %rax
    pop %rcx
    pop %rsp
    // Continue executing the user code
    ret $128


    .global __occlum_syscall_c_abi
    .type __occlum_syscall_c_abi, @function
__occlum_syscall_c_abi:
    movq %rdi,%rax
    movq %rsi,%rdi
    movq %rdx,%rsi
    movq %rcx,%rdx
    movq %r8,%r10
    movq %r9,%r8
    movq 8(%rsp),%r9
    call __occlum_syscall_linux_abi
    ret
