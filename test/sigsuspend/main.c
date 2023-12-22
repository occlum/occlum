#include <sys/select.h>
#include <sys/timerfd.h>
#include <signal.h>
#include <unistd.h>
#include <stdio.h>
#include <errno.h>
#include <stdint.h> // for uint64_t
#include <pthread.h>
#include "test.h"

// Signal handler for SIGUSR1
void sigusr_handler(int sig) {
    printf("Received signals: %d. ", sig);
}

void *send_signal(void *arg) {
    pthread_t main_thread_id = *(pthread_t *)arg;
    sleep(1);
    pthread_kill(main_thread_id, SIGUSR1);
    sleep(1);
    pthread_kill(main_thread_id, SIGUSR2);
    return NULL;
}

int main() {
    // Set SIGUSR1 signal action
    struct sigaction sa1;
    sa1.sa_handler = sigusr_handler;
    sigemptyset(&sa1.sa_mask);
    sa1.sa_flags = 0;
    sigaction(SIGUSR1, &sa1, NULL);

    // Set SIGUSR2 signal action
    struct sigaction sa2;
    sa2.sa_handler = sigusr_handler;
    sigemptyset(&sa2.sa_mask);
    sa2.sa_flags = 0;
    sigaction(SIGUSR2, &sa2, NULL);

    // Mask for blocking SIGUSR1 signal
    sigset_t sigmask;
    sigemptyset(&sigmask);
    sigaddset(&sigmask, SIGUSR1);

    // Access pthread id
    pthread_t main_thread_id = pthread_self();

    // Spawn new thread for sending signal when call pselect syscall
    pthread_t signal_thread;
    if (pthread_create(&signal_thread, NULL, send_signal, &main_thread_id) != 0) {
        THROW_ERROR("failed to create pthread");
        return 1;
    }

    int ret = sigsuspend(&sigmask);
    if (ret == -1) {
        printf("Signal received, the rt_sigsuspend syscall returns successfully\n");
    } else {
        THROW_ERROR("failed to call rt_sigsuspend syscall");
    }

    pthread_join(signal_thread, NULL);
    return 0;
}
