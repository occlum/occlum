#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/time.h>

int main() {
    struct rusage usage;

    if (getrusage(RUSAGE_SELF, &usage) == -1) {
        perror("getrusage failed");
        return EXIT_FAILURE;
    }

    printf("User CPU time used: %ld.%06ld seconds\n",
           usage.ru_utime.tv_sec, usage.ru_utime.tv_usec);

    printf("System CPU time used: %ld.%06ld seconds\n",
           usage.ru_stime.tv_sec, usage.ru_stime.tv_usec);

    return EXIT_SUCCESS;
}
