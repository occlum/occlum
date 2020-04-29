#include <sys/types.h>
#include <sys/stat.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>
#include <errno.h>

int main(int argc, const char* argv[]) {

    const int BUF_SIZE = 20;
    char buf[10];
    int ret;

    char* cwd = getcwd(buf, BUF_SIZE);
    if(cwd != buf) {
        printf("failed to getcwd\n");
        return -1;
    }

    const char expect_cwd[] = "/";
    if(strcmp(buf, expect_cwd)) {
        printf("incorrect cwd \"%s\". expect \"%s\".\n", buf, expect_cwd);
        return -1;
    }

    //const char DIR_NAME[] = "test_dir";
    //const char DIR_PATH[] = "/test_dir";
    const char DIR_NAME[] = "/root/test_dir";
    const char DIR_PATH[] = "/root/test_dir";
    const int DIR_MODE = 0664;
    ret = mkdir(DIR_NAME, DIR_MODE);
    if(ret < 0) {
        printf("failed to mkdir \"%s\"", DIR_NAME);
        return ret;
    }

    ret = chdir(DIR_NAME);
    if(ret < 0) {
        printf("failed to chdir to \"%s\"", DIR_NAME);
        return ret;
    }

    cwd = getcwd(buf, BUF_SIZE);
    if(cwd != buf) {
        printf("failed to getcwd\n");
        return -1;
    }

    if(strcmp(buf, DIR_PATH)) {
        printf("incorrect cwd \"%s\". expect \"%s\".\n", buf, DIR_PATH);
        return -1;
    }

    ret = rmdir(DIR_PATH);
    if(ret < 0) {
        printf("failed to rmdir \"%s\"", DIR_PATH);
        return ret;
    }

    struct stat stat_buf;
    ret = stat(DIR_PATH, &stat_buf);
    if(!(ret < 0 && errno == ENOENT)) {
        printf("stat on \"%s\" should return ENOENT", DIR_PATH);
        return ret;
    }

    printf("getcwd, mkdir, rmdir, chdir test successful\n");
    return 0;
}
