#include "switch.h"
#include <string.h>

#define THIS_SHOULD_NEVER_HAPPEN        while (1) { }

void __switch_to_user(
    CpuContext *user_context,
    jmp_buf jb,
    void *fault
) __attribute__((noreturn));

void _switch_to_user(CpuContext *user_context, void *fault) {
    jmp_buf jb;
    int second = setjmp(jb);
    if (!second) {
        __switch_to_user(user_context, jb, fault);
        THIS_SHOULD_NEVER_HAPPEN;
    }
    // Back from the user space with user_context updated
    return;
}

void _restore_kernel_state(jmp_buf jb) __attribute__((noreturn));
void _restore_kernel_state(jmp_buf jb) {
    longjmp(jb, 1);
    THIS_SHOULD_NEVER_HAPPEN;
}
