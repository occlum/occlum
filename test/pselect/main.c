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
void sigusr1_handler(int sig) {
    printf("SIGUSR1 received\n");
}

void *send_signal(void *arg) {
    pthread_t main_thread_id = *(pthread_t *)arg;
    sleep(1);
    pthread_kill(main_thread_id, SIGUSR1);
    return NULL;
}

int main() {
    struct sigaction sa;
    sa.sa_handler = sigusr1_handler;
    sigemptyset(&sa.sa_mask);
    sa.sa_flags = 0;
    sigaction(SIGUSR1, &sa, NULL);

    sigset_t sigmask;
    sigemptyset(&sigmask);
    sigaddset(&sigmask, SIGUSR1);

    // Access pthread id
    pthread_t main_thread_id = pthread_self();

    // Spawn new thread for sending signal when call pselect syscall
    pthread_t signal_thread;
    if (pthread_create(&signal_thread, NULL, send_signal, &main_thread_id) != 0) {
        THROW_ERROR("pthread_create");
        return 1;
    }

    int timer_fd = timerfd_create(CLOCK_REALTIME, 0);
    if (timer_fd == -1) {
        THROW_ERROR("timerfd_create");
        return 1;
    }

    struct itimerspec timerValue;
    timerValue.it_value.tv_sec = 2;
    timerValue.it_value.tv_nsec = 0;
    timerValue.it_interval.tv_sec = 0;
    timerValue.it_interval.tv_nsec = 0;
    if (timerfd_settime(timer_fd, 0, &timerValue, NULL) == -1) {
        THROW_ERROR("timerfd_settime");
        close(timer_fd);
        return 1;
    }

    fd_set readfds;
    FD_ZERO(&readfds);
    FD_SET(timer_fd, &readfds);

    int ready = pselect(timer_fd + 1, &readfds, NULL, NULL, NULL, &sigmask);

    if (ready > 0) {
        if (FD_ISSET(timer_fd, &readfds)) {
            printf("Timer expired, pselect blocked SIGUSR1 signal successfully\n");
            uint64_t expirations;
            read(timer_fd, &expirations, sizeof(expirations));
        }
    } else if (ready == 0) {
        // Impossible case
        printf("No input - timeout reached\n");
    } else {
        THROW_ERROR("failed to pselect");
    }

    pthread_join(signal_thread, NULL);
    close(timer_fd);

    return 0;
}
