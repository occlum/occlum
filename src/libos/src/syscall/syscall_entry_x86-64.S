#define __ASSEMBLY__
#include "task.h"


    .file "syscall_entry_x86-64.S"
    .global __occlum_syscall_linux_pku_abi
    .type __occlum_syscall_linux_pku_abi, @function
__occlum_syscall_linux_pku_abi:
    pushq %rcx
    pushq %rdx
    pushq %rax

    xor %ecx, %ecx
    xor %edx, %edx
    mov $PKRU_LIBOS, %eax
    wrpkru

    popq %rax
    popq %rdx
    popq %rcx

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
    //      return address - %rcx

    // Save rsp in r11
    // r11 is used to store RFLAGS. Since the FLAGS is not changed before pushfq,
    // r11 is used to save original rsp.
    movq %rsp, %r11

    // Get current task
    movq %gs:(TD_TASK_OFFSET), %rsp
    // Switch to the kernel stack
    movq TASK_KERNEL_RSP(%rsp), %rsp

    // Save the target CPU state when `call __occlum_syscall` is returned in
    // a CpuContext struct. The registers are saved in the reverse order of 
    // the fields in CpuContext.
    pushq $0         // default fpregs is NULL 
    pushq $0         // default fpregs is allocated on stack
    pushfq
    push %rcx       // save %rip
    push %r11       // save %rsp
    push %rcx
    push %rax
    push %rdx
    push %rbx
    push %rbp
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

    // Get current task
    movq %gs:(TD_TASK_OFFSET), %r12

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
    
    // Keep the stack 16 bytes alignment
    and $-16, %rsp

    // Do syscall
    call occlum_syscall

    // This should never happen!
    ud2


    .global __occlum_sysret
    .type __occlum_sysret, @function
__occlum_sysret:
    // Arguments:
    //      %rdi - user_context: &mut CpuContext

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

    mov pku_enabled(%rip), %rax
    cmp $1, %rax
    je update_pkru_in_sysret

    // Restore flags first
    leaq (17*8)(%rdi), %rsp
    popfq

    // Restore the return address
    movq (16*8)(%rdi), %rcx       //save the return address in %rcx
    movq %rcx, %gs:(TD_SYSCALL_RET_ADDR_OFFSET)

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



    jmp *%gs:(TD_SYSCALL_RET_ADDR_OFFSET)
    // This should never happen
    ud2

update_pkru_in_sysret:
    // Arguments:
    //      %rdi - user_context: &mut CpuContext

    // The `push` and `pushfq` instructions after `pop %rsp` may over-written data in red zone
    // of user function and causing a crash.
    // One workaround is to skip the red zone at the top of the stack:
    // 1. Enlarge stack by modifying context->rsp.
    // 2. Store the original %rsp on stack.
    // 3. before we return to user process, pop the original rsp from the stack.

    movq (15*8)(%rdi), %rcx     // read %rsp value from user context
    mov %rcx, %rax              // backup original %rsp value in %rax
    sub (128+8), %rcx           // skip the red zone area (128 bytes according to x86_64 System V ABI)
    movq %rcx, (15*8)(%rdi)     // save modified %rsp value to user context
    movq %rax, (%rcx)           // save original %rsp value to user stack

    // Restore flags first
    leaq (17*8)(%rdi), %rsp
    popfq

    // Restore the return address
    movq (16*8)(%rdi), %rcx       //save the return address in %rcx
    movq %rcx, %gs:(TD_SYSCALL_RET_ADDR_OFFSET)

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

    // Prepare for calling wrpkru
    pushfq
    push %rax
    push %rcx
    push %rdx
    push %gs:(TD_SYSCALL_RET_ADDR_OFFSET)

    mov $0, %ecx
    mov $0, %edx
    mov $PKRU_USER, %eax
    wrpkru

    pop %rdx    // for stack balancing
    pop %rdx
    pop %rcx
    pop %rax
    popfq
    pop %rsp

    jmp *(128+8+5*8)(%rsp)
    // This should never happen
    ud2

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
    lea syscall_return(%rip), %rcx
    jmp __occlum_syscall_linux_abi
syscall_return:
    ret
