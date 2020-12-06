#include "switch.h"
#include <string.h>

#define THIS_SHOULD_NEVER_HAPPEN        while (1) { }

void __switch_to_user(
    CpuContext *user_context,
    uint64_t user_fs,
    jmp_buf jb
) __attribute__((noreturn));

void switch_to_user(CpuContext *user_context, uint64_t user_fs) {
    jmp_buf jb;
    int second = setjmp(jb);
    if (!second) {
        __switch_to_user(user_context, user_fs, jb);
        THIS_SHOULD_NEVER_HAPPEN;
    }
    // Back from the user space with user_context updated
    return;
}


void switch_to_kernel(jmp_buf jb, CpuContext *user_context) __attribute__((noreturn));

void switch_to_kernel(jmp_buf jb, CpuContext *user_context) {
    // Init the two fields that haven't been initialized by the assembly code
    user_context->fpregs_on_heap = 0;
    user_context->fpregs = NULL;

    longjmp(jb, 1);
    THIS_SHOULD_NEVER_HAPPEN;
}
