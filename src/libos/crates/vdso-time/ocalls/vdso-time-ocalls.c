#include <sys/auxv.h>
#include <sys/utsname.h>
#include <time.h>
#include <string.h>

int vdso_ocall_get_vdso_info(
    unsigned long *vdso_addr,
    char *release,
    int release_len) {
    // If AT_SYSINFO_EHDR isn't found, getauxval will return 0.
    *vdso_addr = getauxval(AT_SYSINFO_EHDR);

    struct utsname buf;
    int ret = uname(&buf);
    // uname should always succeed here, since uname only fails when buf is not invalid.
    if (ret != 0) { return -1; }

    strncpy(release, buf.release, release_len);
    release[release_len - 1] = '\0';

    return 0;
}

int vdso_ocall_clock_gettime(int clockid, struct timespec *tp) {
    return clock_gettime(clockid, tp);
}

int vdso_ocall_clock_getres(int clockid, struct timespec *res) {
    return clock_getres(clockid, res);
}
