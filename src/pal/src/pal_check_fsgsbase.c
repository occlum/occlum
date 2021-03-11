#define _GNU_SOURCE
#include <stdio.h>
#include <signal.h>
#include <assert.h>
#include <string.h>
#include <ucontext.h>
#include <setjmp.h>
#include <stdlib.h>
#include <errno.h>

#include "pal_log.h"
#include "pal_check_fsgsbase.h"

#define RC 0xffff
static jmp_buf env_buf;

static void handle_sigill(int num) {
    assert(num == SIGILL);

    longjmp(env_buf, RC);
}

int check_fsgsbase_enablement(void) {
    int gs_read_data = 0;
    int gs_write_data = 0x0f;
    int __seg_gs *offset_ptr = 0;   // offset relative to GS. support since gcc-6

    sighandler_t handler_orig = signal(SIGILL, handle_sigill);
    if (handler_orig == SIG_ERR) {
        PAL_ERROR("registering signal handler failed, errno = %d", errno);
        return errno;
    }

    int ret = setjmp(env_buf);
    if (ret == RC) {
        // return from SIGILL handler
        PAL_ERROR("\tSIGILL Caught !");
        return -1;
    }
    if (ret != 0) {
        PAL_ERROR("setjmp failed");
        return -1;
    }

    // Check if kernel supports FSGSBASE
    asm("rdgsbase %0" :: "r" (&gs_read_data));
    asm("wrgsbase %0" :: "r" (&gs_write_data));

    if (*offset_ptr != 0x0f) {
        PAL_ERROR("GS register data not match\n");
        return -1;
    };

    // Restore the GS register and original signal handler
    asm("wrgsbase %0" :: "r" (&gs_read_data));
    handler_orig = signal(SIGILL, handler_orig);
    if (handler_orig == SIG_ERR) {
        PAL_ERROR("restoring default signal handler failed, errno = %d", errno);
        return errno;
    }

    return 0;
}
