#include "syscall.h"
#include "task.h"

long dispatch_syscall(int num, long arg0, long arg1, long arg2, long arg3, long arg4) {
    long ret = 0;

    switch (num) {
    case SYS_exit:
        do_exit_task((int)arg0);
        break;
    case SYS_write:
        ret = (long) rusgx_write((int)arg0, (const void*)arg1, (size_t)arg2);
        break;
    default:
        ret = -1;
        break;
    }

    return ret;
}
