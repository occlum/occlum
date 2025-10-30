#define _GNU_SOURCE
#include <sys/ipc.h>
#include <sys/sem.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <fcntl.h>
#include <time.h>
#include <stdlib.h>
#include <stdio.h>
#include <spawn.h>
#include <unistd.h>
#include <errno.h>
#include "test.h"

// ============================================================================
// Global Definitions (Semaphore Test Specific)
// ============================================================================

// Permission macro (same as shm, ensures user read/write permissions)
#define S_IRWUSER   (S_IRUSR | S_IWUSR)

// Test type identifiers
#define TEST_GET_SEMID_BY_KEY   0   // Get semid by key
#define TEST_PROCESS_SYNC       1   // Inter-process semaphore synchronization
#define TEST_IMMEDIATELY_RMSEM  2   // Verify immediate semaphore removal
#define TEST_OPERATE_DESTOYED   3   // Operate on destroyed semaphore
#define TEST_NO_RMSEM           4   // Do not remove semaphore (memory leak detection)

// Number of parameters for each test case (child process argv length)
#define TEST_GET_SEMID_BY_KEY_ARGC  5
#define TEST_PROCESS_SYNC_ARGC     4
#define TEST_OPERATE_DESTOYED_ARGC  5

// General macro definitions
#define ARG_BUF_SZ  64              // Parameter buffer size
#define SEM_SET_SIZE 1              // Semaphore set size (using 1 semaphore)
#define SEM_INIT_VAL 1              // Default initial semaphore value
#define SUCCESS     1
#define FAIL        (-1)

const char prog_name[] = "/bin/sem";

// ============================================================================
// Helper Macros and Functions (Reusing shm test logic, adapted for semaphores)
// ============================================================================

// Log printing macro (includes file/line/function name for easier troubleshooting)
#define INFO(fmt, ...)   do { \
    printf("\t\t[SEM TEST] [file: %s, line: %d, func: %s] " fmt, \
    __FILE__, __LINE__, __func__, ##__VA_ARGS__); \
} while (0)

// Spawn child process to execute test and check exit status
// Returns SUCCESS if child test passes; FAIL if child test fails
static int execute_in_child(char **const child_argv) {
    int ret;
    pid_t child_pid;
    int child_status;

    // Create child process
    ret = posix_spawn(&child_pid, prog_name, NULL, NULL, (char **const)child_argv, NULL);
    if (ret < 0) {
        THROW_ERROR("posix_spawn() failed (errno: %d)", errno);
    }

    // Wait for child process to exit
    ret = waitpid(child_pid, &child_status, 0);
    if (ret < 0) {
        THROW_ERROR("waitpid() failed (errno: %d)", errno);
    }

    // Check child process exit status (normal exit with code 0)
    if (!WIFEXITED(child_status) || WEXITSTATUS(child_status) != 0) {
        INFO("Child process test failed (exit code: %d)\n", WEXITSTATUS(child_status));
        return FAIL;
    }

    return SUCCESS;
}

// ============================================================================
// Core Semaphore Test Cases
// ============================================================================

/**
 * Test 1: Create/get semid by key (basic functionality verification)
 * Scenarios:
 * 1. Get non-existent semaphore → should return ENOENT
 * 2. Create new semaphore (IPC_CREAT|IPC_EXCL) → should successfully get semid
 * 3. Get existing semaphore by key in same process → semid should match
 * 4. Attempt to recreate with IPC_EXCL → should return EEXIST
 * 5. Child process gets semaphore by key → semid should match parent's
 */
static int test_semget_semid_from_key(void) {
    int ret, semid;
    key_t key;
    char *child_argv[TEST_GET_SEMID_BY_KEY_ARGC + 1]; // +1 for NULL terminator

    // Generate random key (avoid conflict with existing semaphores)
    srand(time(NULL));
    key = random();
    INFO("Test key: %d\n", key);

    // Scenario 1: Get non-existent semaphore → expect ENOENT
    ret = syscall(SYS_semget, key, SEM_SET_SIZE, S_IRWUSER);
    if (ret != -1 || errno != ENOENT) {
        INFO("semget() failed: expected ENOENT, actual ret=%d, errno=%d\n", ret, errno);
        return FAIL;
    }

    // Scenario 2: Create new semaphore (IPC_CREAT|IPC_EXCL) → should succeed
    semid = syscall(SYS_semget, key, SEM_SET_SIZE, IPC_CREAT | IPC_EXCL | S_IRWUSER);
    if (semid < 0) {
        THROW_ERROR("semget() create failed (errno: %d)", errno);
    }
    INFO("Created semid: %d\n", semid);

    // Scenario 3: Set initial semaphore value (undefined after semget, needs explicit setting)
    ret = syscall(SYS_semctl, semid, 0, SETVAL, SEM_INIT_VAL);
    if (ret < 0) {
        THROW_ERROR("semctl(SETVAL) failed (errno: %d)", errno);
    }


    // Scenario 4: Get existing semaphore in same process → verify semid and value match
    ret = syscall(SYS_semget, key, SEM_SET_SIZE, S_IRWUSER);
    if (ret < 0) {
        THROW_ERROR("semget() get existing failed (errno: %d)", errno);
    }
    if (ret != semid) {
        INFO("semid mismatch: expected %d, actual %d\n", semid, ret);
        return FAIL;
    }
    ret = syscall(SYS_semctl, semid, 0, GETVAL);
    if (ret != SEM_INIT_VAL) {
        INFO("sem val mismatch: expected %d, actual %d\n", SEM_INIT_VAL, ret);
        return FAIL;
    }

    // Scenario 5: Attempt to recreate with IPC_EXCL → expect EEXIST
    ret = syscall(SYS_semget, key, SEM_SET_SIZE, IPC_CREAT | IPC_EXCL | S_IRWUSER);
    if (ret != -1 || errno != EEXIST) {
        INFO("semget() failed: expected EEXIST, actual ret=%d, errno=%d\n", ret, errno);
        return FAIL;
    }

    // Scenario 6: Child process gets semid by key → verify consistency
    // Initialize child process arguments (argv[0]=program name, argv[1]=test type, argv[2]=key, argv[3]=semid, argv[4]=initial value)
    for (int i = 0; i < TEST_GET_SEMID_BY_KEY_ARGC; i++) {
        child_argv[i] = (char *)malloc(ARG_BUF_SZ);
        if (child_argv[i] == NULL) {
            THROW_ERROR("malloc() failed for child argv");
        }
    }
    snprintf(child_argv[0], ARG_BUF_SZ, "%s", prog_name);
    snprintf(child_argv[1], ARG_BUF_SZ, "%d", TEST_GET_SEMID_BY_KEY);
    snprintf(child_argv[2], ARG_BUF_SZ, "%d", key);
    snprintf(child_argv[3], ARG_BUF_SZ, "%d", semid);
    snprintf(child_argv[4], ARG_BUF_SZ, "%d", SEM_INIT_VAL);
    child_argv[TEST_GET_SEMID_BY_KEY_ARGC] = NULL; // Terminator

    // Execute child process test
    if ((ret = execute_in_child(child_argv)) != SUCCESS) {
        INFO("Child test (GET_SEMID_BY_KEY) failed\n");
        return FAIL;
    }

    // Cleanup resources
    for (int i = 0; i < TEST_GET_SEMID_BY_KEY_ARGC; i++) {
        free(child_argv[i]);
    }

    // Delete semaphore
    ret = syscall(SYS_semctl, semid, 0, IPC_RMID);
    if (ret < 0) {
        THROW_ERROR("semctl(IPC_RMID) failed (errno: %d)", errno);
    }

    INFO("Test semget_semid_from_key passed\n");
    return SUCCESS;
}

/**
 * Test 2: Inter-process semaphore synchronization (core functionality verification)
 * Logic:
 * 1. Parent process creates semaphore (initial value 1), performs P operation (value→0)
 * 2. Child process performs V operation (value→1)
 * 3. Parent process verifies semaphore value returns to 1 → synchronization works
 */
static int test_process_sync(void) {
    int ret, semid;
    int sem_val;
    char *child_argv[TEST_PROCESS_SYNC_ARGC + 1];
    // Semaphore operation structure (P operation: sem_op=-1, V operation: sem_op=1)
    struct sembuf p_op = {0, -1, 0}; // sem_num=0 (first semaphore), sem_flg=0 (blocking)

    // Step 1: Create anonymous semaphore (IPC_PRIVATE, visible only to parent and child)
    semid = syscall(SYS_semget, IPC_PRIVATE, SEM_SET_SIZE, IPC_CREAT | IPC_EXCL | S_IRWUSER);
    if (semid < 0) {
        THROW_ERROR("semget() create private failed (errno: %d)", errno);
    }

    // Step 2: Set initial value to 1
    ret = syscall(SYS_semctl, semid, 0, SETVAL, SEM_INIT_VAL);
    if (ret < 0) {
        THROW_ERROR("semctl(SETVAL) failed (errno: %d)", errno);
    }

    // Step 3: Parent performs P operation (value from 1→0)
    ret = syscall(SYS_semop, semid, &p_op, 1); // 1=number of operations
    if (ret < 0) {
        THROW_ERROR("semop(P) failed (errno: %d)", errno);
    }
    // Verify value is 0 after P operation
    sem_val = syscall(SYS_semctl, semid, 0, GETVAL);
    if (sem_val != 0) {
        INFO("P operation failed: expected val=0, actual=%d\n", sem_val);
        return FAIL;
    }

    // Step 4: Create child process to perform V operation
    // Initialize child process arguments (argv[0]=program name, argv[1]=test type, argv[2]=semid, argv[3]=initial value)
    for (int i = 0; i < TEST_PROCESS_SYNC_ARGC; i++) {
        child_argv[i] = (char *)malloc(ARG_BUF_SZ);
        if (child_argv[i] == NULL) {
            THROW_ERROR("malloc() failed for child argv");
        }
    }
    snprintf(child_argv[0], ARG_BUF_SZ, "%s", prog_name);
    snprintf(child_argv[1], ARG_BUF_SZ, "%d", TEST_PROCESS_SYNC);
    snprintf(child_argv[2], ARG_BUF_SZ, "%d", semid);
    snprintf(child_argv[3], ARG_BUF_SZ, "%d", SEM_INIT_VAL);
    child_argv[TEST_PROCESS_SYNC_ARGC] = NULL;

    // Execute child process V operation
    if ((ret = execute_in_child(child_argv)) != SUCCESS) {
        INFO("Child test (PROCESS_SYNC) failed\n");
        return FAIL;
    }

    // Step 5: Verify semaphore value returns to 1 (child's V operation worked)
    sem_val = syscall(SYS_semctl, semid, 0, GETVAL);
    if (sem_val != 1) {
        INFO("Sync failed: expected val=1, actual=%d\n", sem_val);
        return FAIL;
    }

    // Cleanup resources
    for (int i = 0; i < TEST_PROCESS_SYNC_ARGC; i++) {
        free(child_argv[i]);
    }

    // Delete semaphore
    ret = syscall(SYS_semctl, semid, 0, IPC_RMID);
    if (ret < 0) {
        THROW_ERROR("semctl(IPC_RMID) failed (errno: %d)", errno);
    }

    INFO("Test process_sync passed\n");
    return SUCCESS;
}

/**
 * Test 3: Verify immediate semaphore removal (abnormal scenario)
 * Logic: Delete semaphore immediately after creation, subsequent stat operations return EINVAL
 */
static int test_immediately_rmsem(void) {
    int ret, semid;
    struct semid_ds sem_buf; // Semaphore status buffer

    // Step 1: Create semaphore
    semid = syscall(SYS_semget, IPC_PRIVATE, SEM_SET_SIZE, IPC_CREAT | IPC_EXCL | S_IRWUSER);
    if (semid < 0) {
        THROW_ERROR("semget() create failed (errno: %d)", errno);
    }

    // Step 2: Delete semaphore immediately
    ret = syscall(SYS_semctl, semid, 0, IPC_RMID);
    if (ret < 0) {
        THROW_ERROR("semctl(IPC_RMID) failed (errno: %d)", errno);
    }

    // Step 3: Verify stat operation returns EINVAL after deletion (empty buffer)
    ret = syscall(SYS_semctl, semid, 0, IPC_STAT, NULL);
    if (ret != -1 || errno != EINVAL) {
        INFO("semctl(IPC_STAT) failed: expected EINVAL, actual ret=%d, errno=%d\n", ret, errno);
        return FAIL;
    }

    // Step 4: Verify stat operation returns EINVAL after deletion (non-empty buffer)
    ret = syscall(SYS_semctl, semid, 0, IPC_STAT, &sem_buf);
    if (ret != -1 || errno != EINVAL) {
        INFO("semctl(IPC_STAT) failed: expected EINVAL, actual ret=%d, errno=%d\n", ret, errno);
        return FAIL;
    }

    INFO("Test immediately_rmsem passed\n");
    return SUCCESS;
}

/**
 * Test 4: Operate on destroyed semaphore (abnormal scenario)
 * Logic:
 * 1. Parent creates semaphore then immediately deletes it
 * 2. Child attempts to get by key → returns ENOENT
 * 3. Child attempts to operate on deleted semid → returns EINVAL
 */
static int test_operate_destroyed_sem(void) {
    int ret, semid;
    key_t key;
    char *child_argv[TEST_OPERATE_DESTOYED_ARGC + 1];

    // Generate random key
    srand(time(NULL));
    key = random();
    INFO("Test destroyed key: %d\n", key);

    // Step 1: Create semaphore and set initial value
    semid = syscall(SYS_semget, key, SEM_SET_SIZE, IPC_CREAT | IPC_EXCL | S_IRWUSER);
    if (semid < 0) {
        THROW_ERROR("semget() create failed (errno: %d)", errno);
    }
    ret = syscall(SYS_semctl, semid, 0, SETVAL, SEM_INIT_VAL);
    if (ret < 0) {
        THROW_ERROR("semctl(SETVAL) failed (errno: %d)", errno);
    }

    // Step 2: Delete semaphore immediately
    ret = syscall(SYS_semctl, semid, 0, IPC_RMID);
    if (ret < 0) {
        THROW_ERROR("semctl(IPC_RMID) failed (errno: %d)", errno);
    }

    // Step 3: Child tests operations on destroyed semaphore
    // Initialize child process arguments (argv[0]=program name, argv[1]=test type, argv[2]=key, argv[3]=semid, argv[4]=initial value)
    for (int i = 0; i < TEST_OPERATE_DESTOYED_ARGC; i++) {
        child_argv[i] = (char *)malloc(ARG_BUF_SZ);
        if (child_argv[i] == NULL) {
            THROW_ERROR("malloc() failed for child argv");
        }
    }
    snprintf(child_argv[0], ARG_BUF_SZ, "%s", prog_name);
    snprintf(child_argv[1], ARG_BUF_SZ, "%d", TEST_OPERATE_DESTOYED);
    snprintf(child_argv[2], ARG_BUF_SZ, "%d", key);
    snprintf(child_argv[3], ARG_BUF_SZ, "%d", semid);
    snprintf(child_argv[4], ARG_BUF_SZ, "%d", SEM_INIT_VAL);
    child_argv[TEST_OPERATE_DESTOYED_ARGC] = NULL;

    // Execute child process test
    if ((ret = execute_in_child(child_argv)) != SUCCESS) {
        INFO("Child test (OPERATE_DESTROYED) failed\n");
        return FAIL;
    }

    // Cleanup resources
    for (int i = 0; i < TEST_OPERATE_DESTOYED_ARGC; i++) {
        free(child_argv[i]);
    }

    INFO("Test operate_destroyed_sem passed\n");
    return SUCCESS;
}

/**
 * Test 5: Do not delete semaphore (memory leak detection)
 * Note: As the last test case, verifies if LibOS reclaims unexplicitly deleted semaphore resources on exit
 * Logic: Only create semaphore, do not perform IPC_RMID
 */
static int test_no_rmsem(void) {
    int semid;

    // Create semaphore and set initial value (do not delete)
    semid = syscall(SYS_semget, IPC_PRIVATE, SEM_SET_SIZE, IPC_CREAT | IPC_EXCL | S_IRWUSER);
    if (semid < 0) {
        THROW_ERROR("semget() create failed (errno: %d)", errno);
    }
    int ret = syscall(SYS_semctl, semid, 0, SETVAL, SEM_INIT_VAL);
    if (ret < 0) {
        THROW_ERROR("semctl(SETVAL) failed (errno: %d)", errno);
    }

    INFO("Test no_rmsem passed (wait for resource recycle)\n");
    return SUCCESS;
}

// ============================================================================
// Child Process Test Logic (Corresponding to parent process test types)
// ============================================================================

/**
 * Child process: Verify getting semid by key
 */
static int child_test_get_semid_by_key(int argc, const char *argv[]) {
    key_t key;
    int ret, semid, expected_semid, init_val;
    int actual_val;

    // Check parameter count
    if (argc != TEST_GET_SEMID_BY_KEY_ARGC) {
        INFO("Invalid argc: expected %d, actual %d\n", TEST_GET_SEMID_BY_KEY_ARGC, argc);
        return FAIL;
    }

    // Parse parameters
    key = atoi(argv[2]);
    expected_semid = atoi(argv[3]);
    init_val = atoi(argv[4]);
    INFO("Child get key: %d, expected semid: %d\n", key, expected_semid);

    // Get semid by key
    semid = syscall(SYS_semget, key, SEM_SET_SIZE, S_IRWUSER);
    if (semid < 0) {
        THROW_ERROR("Child semget() failed (errno: %d)", errno);
    }

    // Verify semid matches parent's
    if (semid != expected_semid) {
        INFO("Child semid mismatch: expected %d, actual %d\n", expected_semid, semid);
        return FAIL;
    }

    // Verify initial semaphore value is correct
    actual_val = syscall(SYS_semctl, semid, 0, GETVAL);
    if (actual_val != init_val) {
        INFO("Child sem val mismatch: expected %d, actual %d\n", init_val, actual_val);
        return FAIL;
    }

    return SUCCESS;
}

/**
 * Child process: Perform V operation (synchronize with parent)
 */
static int child_test_process_sync(int argc, const char *argv[]) {
    int semid, init_val;
    int ret;
    struct sembuf v_op = {0, 1, 0}; // V operation: sem_op=1

    // Check parameter count
    if (argc != TEST_PROCESS_SYNC_ARGC) {
        INFO("Invalid argc: expected %d, actual %d\n", TEST_PROCESS_SYNC_ARGC, argc);
        return FAIL;
    }

    // Parse parameters
    semid = atoi(argv[2]);
    init_val = atoi(argv[3]);
    INFO("Child sync semid: %d, init val: %d\n", semid, init_val);

    // Perform V operation (value from 0→1)
    ret = syscall(SYS_semop, semid, &v_op, 1);
    if (ret < 0) {
        THROW_ERROR("Child semop(V) failed (errno: %d)", errno);
    }

    // Verify value is 1 after V operation
    int sem_val = syscall(SYS_semctl, semid, 0, GETVAL);
    if (sem_val != 1) {
        INFO("Child V operation failed: expected val=1, actual=%d\n", sem_val);
        return FAIL;
    }

    return SUCCESS;
}

/**
 * Child process: Verify operations on destroyed semaphore
 */
static int child_test_operate_destroyed_sem(int argc, const char *argv[]) {
    key_t key;
    int semid, init_val;
    int ret;
    struct sembuf p_op = {0, -1, 0}; // Attempt P operation

    // Check parameter count
    if (argc != TEST_OPERATE_DESTOYED_ARGC) {
        INFO("Invalid argc: expected %d, actual %d\n", TEST_OPERATE_DESTOYED_ARGC, argc);
        return FAIL;
    }

    // Parse parameters
    key = atoi(argv[2]);
    semid = atoi(argv[3]);
    init_val = atoi(argv[4]);
    INFO("Child destroy test: key=%d, semid=%d\n", key, semid);

    // Scenario 1: Get deleted semaphore by key → expect ENOENT
    ret = syscall(SYS_semget, key, SEM_SET_SIZE, S_IRWUSER);
    if (ret != -1 || errno != ENOENT) {
        INFO("Child semget() failed: expected ENOENT, actual ret=%d, errno=%d\n", ret, errno);
        return FAIL;
    }

    // Scenario 2: Operate on deleted semid → expect EINVAL
    ret = syscall(SYS_semop, semid, &p_op, 1);
    if (ret != -1 || errno != EINVAL) {
        INFO("Child semop() failed: expected EINVAL, actual ret=%d, errno=%d\n", ret, errno);
        return FAIL;
    }

    return SUCCESS;
}

// ============================================================================
// Test Suite Entry
// ============================================================================

// Test case list (test_no_rmsem must be last for memory leak detection)
static test_case_t test_cases[] = {
    TEST_CASE(test_semget_semid_from_key),
    TEST_CASE(test_process_sync),
    TEST_CASE(test_immediately_rmsem),
    TEST_CASE(test_operate_destroyed_sem),
    TEST_CASE(test_no_rmsem),
};

int main(int argc, const char *argv[]) {
    // Parent process: Run test suite
    if (argc == 1) {
        INFO("Start sem test suite (total cases: %d)\n", ARRAY_SIZE(test_cases));
        return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
    }
    // Child process: Execute specified test logic
    else {
        int option = atoi(argv[1]);
        int ret = FAIL;

        switch (option) {
            case TEST_GET_SEMID_BY_KEY:
                ret = child_test_get_semid_by_key(argc, argv);
                break;
            case TEST_PROCESS_SYNC:
                ret = child_test_process_sync(argc, argv);
                break;
            case TEST_OPERATE_DESTOYED:
                ret = child_test_operate_destroyed_sem(argc, argv);
                break;
            default:
                INFO("Invalid test option: %d\n", option);
                ret = FAIL;
        }

        // Child process returns 0 for success, -1 for failure
        return (ret == SUCCESS) ? 0 : -1;
    }
}
