#include <sys/auxv.h>
#include <sys/utsname.h>
#include <time.h>
#include <string.h>

int vdso_ocall_get_vdso_info(
    unsigned long* vdso_addr, 
    long* hres_resolution,
    long* coarse_resolution,
    char* release,
    int release_len,
    struct timespec* tss,
    int tss_len) 
{
    *vdso_addr = getauxval(AT_SYSINFO_EHDR);

    struct timespec tp;
    if (!clock_getres(CLOCK_REALTIME, &tp)) {
        *hres_resolution = tp.tv_nsec;
    } else *hres_resolution = 0;
    if (!clock_getres(CLOCK_REALTIME_COARSE, &tp)) {
        *coarse_resolution = tp.tv_nsec;
    } else *coarse_resolution = 0;

    struct utsname buf;
    int ret = uname(&buf);
    if (ret != 0) return -1;

    int buf_rel_len = strlen(buf.release);
    int len = buf_rel_len > release_len ? release_len : buf_rel_len;
    memcpy(release, buf.release, len);

    clockid_t clockids[] = { CLOCK_REALTIME, CLOCK_MONOTONIC, CLOCK_MONOTONIC_RAW, 
        CLOCK_REALTIME_COARSE, CLOCK_MONOTONIC_COARSE, CLOCK_MONOTONIC_COARSE };
    for (int i = 0; i < sizeof(clockids) / sizeof(clockid_t); ++i) {
        if (tss_len > clockids[i] && clock_gettime(clockids[i], &tss[clockids[i]]) != 0) {
            tss[clockids[i]].tv_sec = 0;
            tss[clockids[i]].tv_nsec = 0;
        }
    }

    return 0;
}
