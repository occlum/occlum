#include <sys/time.h>
#include <stdio.h>

int main(int argc, const char* argv[]) {
    struct timeval tv;
    gettimeofday(&tv, NULL);
    printf("sec = %lu, usec = %lu\n", tv.tv_sec, tv.tv_usec);
    return 0;
}
