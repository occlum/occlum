#define _GNU_SOURCE
#include <sys/ipc.h>
#include <sys/shm.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <fcntl.h>
#include <time.h>
#include <stdlib.h>
#include <stdio.h>
#include <spawn.h>
#include <unistd.h>
#include "test.h"

// ============================================================================
// Global definitions
// ============================================================================

#define S_IRWUSER   (S_IRUSR | S_IWUSR)

#define TEST_GET_SHMID_BY_KEY   0
#define TEST_PROCESS_COMMU      1
#define TEST_OPERATE_DESTOYED   2

#define TEST_GET_SHMID_BY_KEY_ARGC  5
#define TEST_PROCESS_COMMU_ARGC     4
#define TEST_OPERATE_DESTOYED_ARGC  5

#define ARG_BUF_SZ  64
#define PAGE_SIZE   (0x1000)

#define SUCCESS     1
#define FAIL        (-1)

const char prog_name[] = "/bin/shm";

// ============================================================================
// Helper macro and function
// ============================================================================
#define INFO(fmt, ...)   do { \
    printf("\t\t[file: %s, line: %d, func: %s] " fmt, \
    __FILE__, __LINE__, __func__, ##__VA_ARGS__); \
} while (0)

// Spawn child, and check the return value from child
// Return `SUCCESS` if the execution succeeds,
// return `FAIL` if fails
static int execute_in_child(char **const child_argv) {
    int ret;
    pid_t child_pid;
    int child_status;

    ret = posix_spawn(&child_pid, prog_name, NULL, NULL, (char **const)child_argv, NULL);
    if (ret < 0) {
        THROW_ERROR("Failed to spawn a child process");
    }
    ret = waitpid(child_pid, &child_status, 0);
    if (ret < 0) {
        THROW_ERROR("Failed to waitpid() for child process");
    }
    if (!WIFEXITED(child_status) || WEXITSTATUS(child_status) != 0) {
        INFO("The test in child failed\n");
        return FAIL;
    }

    return SUCCESS;
}

// ============================================================================
// Test cases for shm
// ============================================================================
static int test_shmget_shmid_from_key(void) {
    int ret, shmid;
    key_t key;
    size_t shm_size = PAGE_SIZE;
    char *child_argv[TEST_GET_SHMID_BY_KEY_ARGC + 1];

    srand(time(NULL));
    key = random();
    // Get a non-existent shm segment, should get the return value with ENOENT
    ret = syscall(SYS_shmget, key, shm_size, S_IRWUSER);
    if (ret != -1 || errno != ENOENT) {
        INFO("shmget() should return ENOENT because the segment does not exist, ret: %d errno: %d\n",
             ret, errno);
        return FAIL;
    }

    // Create a new shm segment
    shmid = syscall(SYS_shmget, key, shm_size, IPC_CREAT | IPC_EXCL | S_IRWUSER);
    if (shmid < 0) {
        THROW_ERROR("shmget() cannot create the shm");
    }

    // Get the shmid by key in the same process
    ret = syscall(SYS_shmget, key, shm_size, S_IRWUSER);
    if (ret < 0) {
        THROW_ERROR("shmget() cannot get the shm");
    }
    if (ret != shmid) {
        INFO("shmid mismatches, correct: %d actual: %d\n", shmid, ret);
        return FAIL;
    }

    // Create a shm segment whose key is already attached to existed segment,
    // should get the return value with EEXIST
    ret = syscall(SYS_shmget, key, shm_size, IPC_CREAT | IPC_EXCL | S_IRWUSER);
    if (ret != -1 || errno != EEXIST) {
        INFO("shmget() should return EEXIST because the segment already exists, ret: %d errno: %d\n",
             ret, errno);
        return FAIL;
    }

    // Spawn a new process
    for (int i = 0; i < TEST_GET_SHMID_BY_KEY_ARGC; i++) {
        child_argv[i] = (char *)malloc(ARG_BUF_SZ);
    }
    snprintf(child_argv[0], ARG_BUF_SZ, "%s", prog_name);
    snprintf(child_argv[1], ARG_BUF_SZ, "%d", TEST_GET_SHMID_BY_KEY);
    snprintf(child_argv[2], ARG_BUF_SZ, "%d", key);
    snprintf(child_argv[3], ARG_BUF_SZ, "%d", shmid);
    snprintf(child_argv[4], ARG_BUF_SZ, "%ld", shm_size);
    child_argv[TEST_GET_SHMID_BY_KEY_ARGC] = NULL;
    if ((ret = execute_in_child(child_argv)) != SUCCESS) {
        return FAIL;
    }
    for (int i = 0; i < TEST_GET_SHMID_BY_KEY_ARGC; i++) {
        free(child_argv[i]);
    }

    ret = syscall(SYS_shmctl, shmid, IPC_RMID, NULL);
    if (ret < 0) {
        THROW_ERROR("Cannot remove the segment");
    }
    return SUCCESS;
}

static int test_process_communication() {
    int shmid, ret;
    size_t shm_size = PAGE_SIZE;
    long random_num, *shm_addr;
    char *child_argv[TEST_PROCESS_COMMU_ARGC + 1];

    shmid = syscall(SYS_shmget, IPC_PRIVATE, shm_size, IPC_CREAT | IPC_EXCL | S_IRWUSER);
    if (shmid < 0) {
        THROW_ERROR("shmget() cannot get the shm");
    }
    shm_addr = (long *)syscall(SYS_shmat, shmid, NULL, 0);
    if (shm_addr == (long *) -1) {
        THROW_ERROR("shmat() cannot attach the shm");
    }
    srandom(time(NULL));
    random_num = random();
    *shm_addr = random_num;

    // Spawn a new process
    for (int i = 0; i < TEST_PROCESS_COMMU_ARGC; i++) {
        child_argv[i] = (char *)malloc(ARG_BUF_SZ);
    }
    snprintf(child_argv[0], ARG_BUF_SZ, "%s", prog_name);
    snprintf(child_argv[1], ARG_BUF_SZ, "%d", TEST_PROCESS_COMMU);
    snprintf(child_argv[2], ARG_BUF_SZ, "%d", shmid);
    snprintf(child_argv[3], ARG_BUF_SZ, "%ld", random_num);
    child_argv[TEST_PROCESS_COMMU_ARGC] = NULL;
    if ((ret = execute_in_child(child_argv)) != SUCCESS) {
        return FAIL;
    }
    for (int i = 0; i < TEST_PROCESS_COMMU_ARGC; i++) {
        free(child_argv[i]);
    }

    ret = syscall(SYS_shmdt, (void *)shm_addr);
    if (ret != 0) {
        THROW_ERROR("shmdt() failed");
    }

    ret = syscall(SYS_shmctl, shmid, IPC_RMID, NULL);
    if (ret < 0) {
        THROW_ERROR("Cannot remove the segment");
    }

    return SUCCESS;
}

static int test_immediately_rmshm() {
    int ret, shmid;
    size_t shm_size = PAGE_SIZE;
    struct shmid_ds buf;

    shmid = syscall(SYS_shmget, IPC_PRIVATE, shm_size, IPC_CREAT | IPC_EXCL | S_IRWUSER);
    if (shmid < 0) {
        THROW_ERROR("shmget() cannot get the shm");
    }

    ret = syscall(SYS_shmctl, shmid, IPC_RMID, NULL);
    if (ret < 0) {
        THROW_ERROR("Cannot remove the segment");
    }
    ret = syscall(SYS_shmctl, shmid, IPC_STAT, NULL);
    if (ret != -1 || errno != EINVAL) {
        INFO("Should get errno with EINVAL even though the buf is empty, ret: %d errno: %d\n",
             ret, errno);
        return FAIL;
    }
    ret = syscall(SYS_shmctl, shmid, IPC_STAT, &buf);
    if (ret != -1 || errno != EINVAL) {
        INFO("The shared memory segment should be removed immediately since shm_nattach equals to 0, ret: %d errno: %d\n",
             ret, errno);
        return FAIL;
    }

    return SUCCESS;
}

static int test_operate_destroyed_shm() {
    int shmid, ret;
    size_t shm_size = PAGE_SIZE;
    void *shm_addr;
    key_t key;
    char *child_argv[TEST_OPERATE_DESTOYED_ARGC + 1];

    srand(time(NULL));
    key = random();
    shmid = syscall(SYS_shmget, key, shm_size, IPC_CREAT | IPC_EXCL | S_IRWUSER);
    if (shmid < 0) {
        THROW_ERROR("shmget() cannot get the shm");
    }
    shm_addr = (void *)syscall(SYS_shmat, shmid, NULL, 0);
    if (shm_addr == (void *) -1) {
        THROW_ERROR("shmat() cannot attach the shm");
    }

    // Mark the shared memory segment to be destroyed first
    ret = syscall(SYS_shmctl, shmid, IPC_RMID, NULL);

    // Spawn a new process
    for (int i = 0; i < TEST_OPERATE_DESTOYED_ARGC; i++) {
        child_argv[i] = (char *)malloc(ARG_BUF_SZ);
    }
    snprintf(child_argv[0], ARG_BUF_SZ, "%s", prog_name);
    snprintf(child_argv[1], ARG_BUF_SZ, "%d", TEST_OPERATE_DESTOYED);
    snprintf(child_argv[2], ARG_BUF_SZ, "%d", key);
    snprintf(child_argv[3], ARG_BUF_SZ, "%ld", shm_size);
    snprintf(child_argv[4], ARG_BUF_SZ, "%d", shmid);
    child_argv[TEST_OPERATE_DESTOYED_ARGC] = NULL;
    if ((ret = execute_in_child(child_argv)) != SUCCESS) {
        return FAIL;
    }
    for (int i = 0; i < TEST_OPERATE_DESTOYED_ARGC; i++) {
        free(child_argv[i]);
    }

    ret = syscall(SYS_shmdt, shm_addr);

    return SUCCESS;
}

// test_no_rmshm() should be place as the last test case
//
// Occlum checks whether all the memory segment is recycled when the LibOS exits,
// to detect and prevent memory leaks.
// Such test checks whether all the vmas allocated by `shm mod` are recycled when the LibOS exits,
// even though no IPC_RMID is invoked for the shared memory segment.
static int test_no_rmshm() {
    int shmid;
    size_t shm_size = PAGE_SIZE;
    void *shm_addr;

    shmid = syscall(SYS_shmget, IPC_PRIVATE, shm_size, IPC_CREAT | IPC_EXCL | S_IRWUSER);
    if (shmid < 0) {
        THROW_ERROR("shmget() cannot get the shm");
    }
    shm_addr = (void *)syscall(SYS_shmat, shmid, NULL, 0);
    if (shm_addr == (long *) -1) {
        THROW_ERROR("shmat() cannot attach the shm");
    }

    return SUCCESS;
}

// ============================================================================
// Funtion invoked in child process for inter-process communication
// ============================================================================

static int child_test_get_shmid_by_key(int argc, const char *argv[]) {
    key_t key;
    int ret, shmid;
    size_t shm_size;

    if (argc != TEST_GET_SHMID_BY_KEY_ARGC) {
        INFO("Invalid argument, argc: %d\n", argc);
        return FAIL;
    }
    key = atoi(argv[2]);
    shmid = atoi(argv[3]);
    shm_size = atol(argv[4]);

    // Get the shmid by the key input from parent process
    ret = syscall(SYS_shmget, key, shm_size, S_IRWUSER);
    if (ret < 0) {
        THROW_ERROR("shmget() cannot get the shm");
    }
    if (ret != shmid) {
        INFO("shmid get in child process mismatches that in parent process, correct: %d actual: %d\n",
             shmid, key);
        return FAIL;
    }

    return SUCCESS;
}

static int child_test_process_communication(int argc, const char *argv[]) {
    int shmid, ret;
    long random_num, *shm_ptr;

    if (argc != TEST_PROCESS_COMMU_ARGC) {
        INFO("Invalid argument, argc: %d\n", argc);
        return FAIL;
    }
    shmid = atoi(argv[2]);
    random_num = atol(argv[3]);

    // Check whether the value in the shared memory segment equals to
    // that written in parent process
    shm_ptr = (long *)syscall(SYS_shmat, shmid, NULL, 0);
    if (shm_ptr == (long *) -1) {
        THROW_ERROR("shmat() cannot attach to the shm");
    }
    if (*shm_ptr != random_num) {
        INFO("Data in shm mismatches, correct: %ld actual: %ld\n", random_num, *shm_ptr);
        return FAIL;
    }

    ret = syscall(SYS_shmdt, (void *)shm_ptr);
    if (ret != 0) {
        THROW_ERROR("shmdt() failed");
    }

    return SUCCESS;
}

static int child_test_operate_destroyed_shm(int argc, const char *argv[]) {
    int shmid, ret;
    key_t key;
    size_t shm_size;
    void *shm_addr;

    if (argc != TEST_OPERATE_DESTOYED_ARGC) {
        INFO("Invalid argument, argc: %d\n", argc);
        return FAIL;
    }
    key = atoi(argv[2]);
    shm_size = atol(argv[3]);
    shmid = atoi(argv[4]);

    ret = syscall(SYS_shmget, key, shm_size, S_IRWUSER);
    if (ret != -1 || errno != ENOENT) {
        INFO("shmget() should return ENOENT because the segment is marked to be destoyed, ret: %d errno: %d\n",
             ret, errno);
        return FAIL;
    }

    shm_addr = (void *)syscall(SYS_shmat, shmid, NULL, 0);
    if (shm_addr == (void *) -1) {
        THROW_ERROR("shmat() cannot attach the shm");
    }

    if ((ret = syscall(SYS_shmdt, shm_addr)) != 0) {
        THROW_ERROR("shmdt() failed");
    }

    return SUCCESS;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_shmget_shmid_from_key),
    TEST_CASE(test_process_communication),
    TEST_CASE(test_immediately_rmshm),
    TEST_CASE(test_operate_destroyed_shm),
    // test_no_rmshm() should be place as the last test case
    TEST_CASE(test_no_rmshm),
};

int main(int argc, const char *argv[]) {
    if (argc == 1) {
        // Parent process will arrive here
        return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
    } else {
        // Child process will arrive here
        int option = atoi(argv[1]), ret;
        switch (option) {
            case TEST_GET_SHMID_BY_KEY:
                ret = child_test_get_shmid_by_key(argc, argv);
                break;
            case TEST_PROCESS_COMMU:
                ret = child_test_process_communication(argc, argv);
                break;
            case TEST_OPERATE_DESTOYED:
                ret = child_test_operate_destroyed_shm(argc, argv);
                break;
            default:
                INFO("Invalid option: %d\n", option);
                ret = FAIL;
        }
        if (ret == SUCCESS) {
            return 0;
        } else {
            return -1;
        }
    }
}
