#include "pal_sig_handler.h"
#include "pal_log.h"
#include <signal.h>
#include <string.h>

// Signal 64 is used to notify interrupts
#define SIGRT_INTERRUPT     64

int pal_register_sig_handlers(void) {
    if (signal(SIGRT_INTERRUPT, SIG_IGN) == SIG_ERR) {
        PAL_ERROR("Failed to register the SIG64 handler");
        return -1;
    }

    if (signal(SIGPIPE, SIG_IGN) == SIG_ERR) {
        PAL_ERROR("Failed to register the SIGPIPE handler");
        return -1;
    }
    return 0;
}
