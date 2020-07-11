#include "pal_sig_handler.h"
#include "pal_log.h"
#include <signal.h>
#include <string.h>

// Signal 64 is used to notify interrupts
#define SIGRT_INTERRUPT     64

int pal_register_sig_handlers(void) {
    struct sigaction action;
    action.sa_handler = SIG_IGN;
    memset(&action.sa_mask, 0, sizeof(action.sa_mask));
    action.sa_flags = 0;
    if (sigaction(SIGRT_INTERRUPT, &action, NULL) < 0) {
        PAL_ERROR("Failed to regiter signal handlers");
        return -1;
    }
    return 0;
}
