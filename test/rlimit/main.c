#include <sys/resource.h>
#include <stdio.h>

int main(int argc, const char *argv[]) {
    struct rlimit rlim;
    if (getrlimit(RLIMIT_AS, &rlim) < 0) {
        printf("ERROR: getrlimit failed\n");
        return -1;
    }
    if (setrlimit(RLIMIT_AS, &rlim) < 0) {
        printf("ERROR: getrlimit failed\n");
        return -1;
    }
    return 0;
}
