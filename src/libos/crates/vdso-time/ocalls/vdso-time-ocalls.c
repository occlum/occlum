#include <sys/auxv.h>
#include <sys/utsname.h>
#include <time.h>
#include <string.h>

int ocall_get_vdso_info(
    unsigned long* vdso_addr, 
    long* coarse_resolution,
    char* release,
    int release_len) 
{
    *vdso_addr = getauxval(AT_SYSINFO_EHDR);

    struct timespec tp;
    if (!clock_getres(CLOCK_REALTIME_COARSE, &tp)) {
        *coarse_resolution = tp.tv_nsec;
    } else *coarse_resolution = 0;

    struct utsname buf;
    int ret = uname(&buf);
    if (ret != 0) return -1;

    int buf_rel_len = strlen(buf.release);
    int len = buf_rel_len > release_len ? release_len : buf_rel_len;
    memcpy(release, buf.release, len);
    return 0;
}
