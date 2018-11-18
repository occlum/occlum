#include "syscall.h"
#include "task.h"

#define DECL_SYSCALL_ARG(_type, _name, _arg)        \
    _type _name = (_type) (_arg)

long dispatch_syscall(int num, long arg0, long arg1, long arg2, long arg3, long arg4) {
    long ret = 0;

    switch (num) {
    case SYS_exit: {
        DECL_SYSCALL_ARG(int, status, arg0);
        occlum_exit(status);
        do_exit_task();
        break;
    }
    case SYS_open: {
        DECL_SYSCALL_ARG(const void*, path, arg0);
        DECL_SYSCALL_ARG(int, flags, arg1);
        DECL_SYSCALL_ARG(int, mode, arg2);
        ret = occlum_open(path, flags, mode);
        break;
    }
    case SYS_close: {
        DECL_SYSCALL_ARG(int, fd, arg0);
        ret = occlum_close(fd);
        break;
    }
    case SYS_write: {
        DECL_SYSCALL_ARG(int, fd, arg0);
        DECL_SYSCALL_ARG(const void*, buf, arg1);
        DECL_SYSCALL_ARG(size_t, buf_size, arg2);
        ret = occlum_write(fd, buf, buf_size);
        break;
    }
    case SYS_read: {
        DECL_SYSCALL_ARG(int, fd, arg0);
        DECL_SYSCALL_ARG(void*, buf, arg1);
        DECL_SYSCALL_ARG(size_t, buf_size, arg2);
        ret = occlum_read(fd, buf, buf_size);
        break;
    }
    case SYS_spawn: {
        DECL_SYSCALL_ARG(int*, child_pid, arg0);
        DECL_SYSCALL_ARG(const char*, path, arg1);
        DECL_SYSCALL_ARG(const char**, argv, arg2);
        DECL_SYSCALL_ARG(const char**, envp, arg3);
        ret = occlum_spawn(child_pid, path, argv, envp);
        break;
    }
    case SYS_wait4: {
        DECL_SYSCALL_ARG(int, child_pid, arg0);
        DECL_SYSCALL_ARG(int*, status, arg1);
        DECL_SYSCALL_ARG(int, options, arg2);
        //DECL_SYSCALL_ARG(struct rusage*, rusage, arg3);
        ret = occlum_wait4(child_pid, status, options/*, rusage*/);
        break;
    }
    case SYS_getpid: {
        ret = occlum_getpid();
        break;
    }
    default:
        ret = -1;
        break;
    }

    return ret;
}
