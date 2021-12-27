#include <signal.h>
#include <pthread.h>
#include "Enclave_u.h"
#include "pal_enclave.h"
#include "pal_error.h"
#include "pal_interrupt_thread.h"
#include "pal_log.h"
#include "pal_syscall.h"
#include "pal_thread_counter.h"
#include "pal_vcpu_thread.h"
#include "errno2str.h"

#define MS                      (1000*1000L) // 1ms = 1,000,000ns
// real-time signal 64 is used to notify interrupts
#define INTERRUPT_SIGNAL        (64)

static pthread_t thread;
static int is_running = 0;

extern pthread_t *pal_vcpu_threads;
extern struct occlum_pal_vcpu_data *pal_vcpu_data;

static void *thread_func(void *_data) {
    unsigned int *switch_cnts = calloc(pal_num_vcpus, sizeof(unsigned int));
    if (switch_cnts == NULL) {
        errno = ENOMEM;
        return NULL;
    }

    while (1) {
        struct timespec timeout = { .tv_sec = 0, .tv_nsec = 250 * MS };
        int counter = pal_thread_counter_wait_zero(&timeout);
        if (counter == 0) {
            free(switch_cnts);
            return NULL;
        }

        for (int vcpu_i = 0; vcpu_i < pal_num_vcpus; vcpu_i++) {
            pthread_t vcpu_thread = pal_vcpu_threads[vcpu_i];
            struct occlum_pal_vcpu_data pal_data = pal_vcpu_data[vcpu_i];

            /*
            * For each VCPU, every context switch (in libos) to userspace
            * adds 1 to switch count. Reset it to 0 after switching back.
            * The logic here is if the switch count keeps no change, one task in
            * this VCPU is somehow blocked in userspace. Thus a wake-up interrupt
            * signal sent is expected to force the blocked VCPU task to yield.
            */
            if ( pal_data.user_space_mark != 0 ) {
                /*PAL_DEBUG("vcpu %d: previous: %d, current %d\n",
                    vcpu_i, switch_cnts[vcpu_i], pal_data.user_space_mark);*/
                if ( pal_data.user_space_mark == switch_cnts[vcpu_i] ) {
                    pthread_kill(vcpu_thread, INTERRUPT_SIGNAL);
                }
                switch_cnts[vcpu_i] = pal_data.user_space_mark;
            }
        }
    }
}

int pal_interrupt_thread_start(void) {
    if (is_running) {
        errno = EEXIST;
        PAL_ERROR("The interrupt thread is already running: %s", errno2str(errno));
        return -1;
    }

    is_running = 1;
    pal_thread_counter_inc();

    int ret = 0;
    if ((ret = pthread_create(&thread, NULL, thread_func, NULL))) {
        is_running = 0;
        pal_thread_counter_dec();

        errno = ret;
        PAL_ERROR("Failed to start the interrupt thread: %s", errno2str(errno));
        return -1;
    }
    return 0;
}

int pal_interrupt_thread_stop(void) {
    if (!is_running) {
        errno = ENOENT;
        return -1;
    }

    is_running = 0;
    pal_thread_counter_dec();

    int ret = 0;
    if ((ret = pthread_join(thread, NULL))) {
        errno = ret;
        PAL_ERROR("Failed to free the interrupt thread: %s", errno2str(errno));
        return -1;
    }

    return 0;
}
