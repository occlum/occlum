#include "ocalls.h"

int occlum_ocall_tkill(int tid, int signum) {
    int tgid = getpid();
    int ret = TGKILL(tgid, tid, signum);
    return ret;
}
