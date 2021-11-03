#include <sys/random.h>
#include "test.h"

// ============================================================================
// Test cases for getrandom
// ============================================================================

int test_getrandom() {
    unsigned long rand;

    ssize_t len = getrandom(&rand, sizeof(unsigned long), GRND_NONBLOCK);
    if (len < 0 || len != sizeof(unsigned long)) {
        THROW_ERROR("failed to call getrandom");
    }
    printf("generate random value: %lu\n", rand);

    return 0;
}

int test_getrandom_blocking() {
    int rand;

    ssize_t len = getrandom(&rand, sizeof(int), 0);
    if (len < 0 || len != sizeof(int)) {
        THROW_ERROR("failed to call getrandom");
    }
    printf("generate random value: %d\n", rand);

    return 0;
}

// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_getrandom),
    TEST_CASE(test_getrandom_blocking),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
