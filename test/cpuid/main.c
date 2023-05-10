#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "test.h"

// ============================================================================
// Helper struct & functions for cpuid
// ============================================================================

typedef struct t_cpuid {
    unsigned int eax;
    unsigned int ebx;
    unsigned int ecx;
    unsigned int edx;
} t_cpuid_t;

static inline void native_cpuid(int leaf, int subleaf, t_cpuid_t *p) {
    memset(p, 0, sizeof(*p));
    /* ecx is often an input as well as an output. */
    asm volatile("cpuid"
                 : "=a" (p->eax),
                 "=b" (p->ebx),
                 "=c" (p->ecx),
                 "=d" (p->edx)
                 : "a" (leaf), "c" (subleaf));
}

static bool is_cpuidinfo_equal(int leaf, t_cpuid_t *cpu, t_cpuid_t *cpu_sgx) {
    /* Leaf 01H CPUID.EBX is related with logical processor. */
    if (leaf == 1) {
        return ((cpu->eax == cpu_sgx->eax) &&
                (cpu->ecx == cpu_sgx->ecx) &&
                (cpu->edx == cpu_sgx->edx));
    }
    /* Leaf 1FH, 0BH and 06H CPUID.EDX is related with logical processor. */
    if (leaf == 0x1F || leaf == 0xB || leaf == 0x6) {
        return ((cpu->eax == cpu_sgx->eax) &&
                (cpu->ebx == cpu_sgx->ebx) &&
                (cpu->ecx == cpu_sgx->ecx));
    }
    return ((cpu->eax == cpu_sgx->eax) &&
            (cpu->ebx == cpu_sgx->ebx) &&
            (cpu->ecx == cpu_sgx->ecx) &&
            (cpu->edx == cpu_sgx->edx));
}

static int g_max_basic_leaf = 0;
static int g_max_extend_leaf = 0;
static bool g_sgx_supported = true;

#define SGX_LEAF 0x12
#define CPUID_FEATURE_FLAGS 0x7
#define SGX_FEATURE_SHIFT 2
#define SGX1_SHIFT 0

static bool is_sgx_supported(void) {
    t_cpuid_t cpu;

    // check sgx feature supported
    native_cpuid(CPUID_FEATURE_FLAGS, 0, &cpu);
    if (!(cpu.ebx & (1 << SGX_FEATURE_SHIFT))) {
        return false;
    }

    // check sgx1 supported
    native_cpuid(SGX_LEAF, 0, &cpu);
    if (!(cpu.eax & (1 << SGX1_SHIFT))) {
        return false;
    }

    return true;
}

#define SKIP_IF_SGX_NOT_SUPPORTED() do { \
    if (!g_sgx_supported) { \
        printf("Warning: SGX is not supported. Skip %s\n", __func__); \
        return 0; \
    } \
} while (0)

// ============================================================================
// Test cases for cpuid
// ============================================================================

static int test_cpuid_with_basic_leaf_zero() {
    t_cpuid_t cpu;
    int leaf = 0;
    int subleaf = 0;

    native_cpuid(leaf, subleaf, &cpu);

    // check if max basic leaf is valid
    if (cpu.eax < 0 || cpu.eax >= 0xFF) {
        THROW_ERROR("max basic leaf is invalid");
    }
    g_max_basic_leaf = cpu.eax;
    return 0;
}

static int test_cpuid_with_basic_leaf_zero_with_subleaf() {
    t_cpuid_t cpu;
    int leaf = 0;
    int subleaf = 256;

    native_cpuid(leaf, subleaf, &cpu);
    if (cpu.eax != g_max_basic_leaf) {
        THROW_ERROR("failed to call cpuid with eax=0 and subleaf");
    }
    return 0;
}

static int test_cpuid_with_extend_leaf_zero() {
    t_cpuid_t cpu;
    int leaf = 0x80000000;
    int subleaf = 0;

    native_cpuid(leaf, subleaf, &cpu);
    if (cpu.eax < 0x80000000) {
        THROW_ERROR("failed to call cpuid with eax=0x80000000");
    }
    g_max_extend_leaf = cpu.eax;
    return 0;
}

static int test_cpuid_with_extend_leaf_zero_with_subleaf() {
    t_cpuid_t cpu;
    int leaf = 0x80000000;
    int subleaf = 256;

    native_cpuid(leaf, subleaf, &cpu);
    if (cpu.eax != g_max_extend_leaf) {
        THROW_ERROR("failed to call cpuid with eax=0x80000000");
    }
    return 0;
}

static int test_cpuid_with_basic_leaf_one() {
    t_cpuid_t cpu;
    int leaf = 0x1;
    int subleaf = 0;

    native_cpuid(leaf, subleaf, &cpu);
    printf("Stepping %d\n", cpu.eax & 0xF); // Bit 3-0
    printf("Model %d\n", (cpu.eax >> 4) & 0xF); // Bit 7-4
    printf("Family %d\n", (cpu.eax >> 8) & 0xF); // Bit 11-8
    printf("Processor Type %d\n", (cpu.eax >> 12) & 0x3); // Bit 13-12
    printf("Extended Model %d\n", (cpu.eax >> 16) & 0xF); // Bit 19-16
    printf("Extended Family %d\n", (cpu.eax >> 20) & 0xFF); // Bit 27-20
    if (cpu.eax == 0) {
        THROW_ERROR("faild to call cpuid with eax=1");
    }
    return 0;
}

static int test_cpuid_with_sgx_verify() {
    t_cpuid_t cpu;
    int leaf = CPUID_FEATURE_FLAGS;
    int subleaf = 0;

    SKIP_IF_SGX_NOT_SUPPORTED();

    native_cpuid(leaf, subleaf, &cpu);
    //CPUID.(EAX=07H, ECX=0H):EBX.SGX = 1,
    // Bit 02: SGX. Supports Intel® Software Guard Extensions (Intel® SGX Extensions) if 1.
    if (((cpu.ebx >> 2) & 0x1) != 1) {
        THROW_ERROR("failed to call cpuid to verify sgx");
    }
    return 0;
}

static int test_cpuid_with_sgx_enumeration() {
    t_cpuid_t cpu;
    int leaf = SGX_LEAF;
    int subleaf = 0;

    SKIP_IF_SGX_NOT_SUPPORTED();

    native_cpuid(leaf, subleaf, &cpu);
    printf("Sgx 1 supported: %d\n", cpu.eax & 0x1);
    printf("Sgx 2 supported: %d\n", (cpu.eax >> 1) & 0x1);
    if (((cpu.eax & 0x1) | ((cpu.eax >> 1) & 0x1)) == 0) {
        THROW_ERROR("failed to call cpuid to get SGX Capbilities");
    }
    if (((cpu.edx & 0xFF) | ((cpu.edx >> 8) & 0xFF)) == 0) {
        THROW_ERROR("get MaxEnclaveSize failed");
    }
    leaf = SGX_LEAF;
    subleaf = 1;
    native_cpuid(leaf, subleaf, &cpu);
    if ((cpu.eax | cpu.ebx | cpu.ecx | cpu.edx) == 0) {
        THROW_ERROR("failed to call cpuid to get SGX Attributes");
    }
    return 0;
}

static int test_cpuid_with_invalid_leaf() {
    t_cpuid_t cpu;
    int leaf[] = {0x8, 0xC, 0xE, 0x11};
    int subleaf = 0;

    for (int i = 0; i < sizeof(leaf) / sizeof(leaf[0]); i++) {
        if (leaf[i] > g_max_basic_leaf) {
            printf("Warning: test leaf 0x%x is greater than CPU max basic leaf. Skipped.\n", leaf[i]);
            continue;
        }

        native_cpuid(leaf[i], subleaf, &cpu);
        if (cpu.eax | cpu.ebx | cpu.ecx | cpu.edx) {
            THROW_ERROR("faild to call cpuid with invalid leaf 0x%x", leaf[i]);
        }
    }
    return 0;
}

static int test_cpuid_with_oversized_leaf() {
    t_cpuid_t cpu;
    int leaf = g_max_extend_leaf + 1;
    int subleaf = 1;
    native_cpuid(leaf, subleaf, &cpu);

    t_cpuid_t cpu_max;
    leaf = g_max_basic_leaf;
    subleaf = 1;
    native_cpuid(leaf, subleaf, &cpu_max);

    if ((cpu.eax != cpu_max.eax) || (cpu.ebx != cpu_max.ebx) ||
            (cpu.ecx != cpu_max.ecx) || (cpu.edx != cpu_max.edx)) {
        THROW_ERROR("failed to call cpuid with oversize leaf");
    }
    return 0;
}

static int test_cpuid_with_random_leaf() {
    t_cpuid_t cpu;
    srand((int)time(NULL));
    int leaf = 0;
    int subleaf = 0;

    for (int i = 0; i < 5; i++) {
        leaf = rand();
        subleaf = rand();
        native_cpuid(leaf, subleaf, &cpu);
        printf("random leaf:%x, subleaf:%x \n", leaf, subleaf);
        printf("eax: %x ebx: %x ecx: %x edx: %x\n", cpu.eax, cpu.ebx, cpu.ecx, cpu.edx);
    }
    return 0;
}

#define BUFF_SIZE (1024)
static int test_cpuid_with_host_cpuidinfo() {
    char buff[BUFF_SIZE] = {0};
    FILE *fp = fopen("./test_cpuid.txt", "r");
    if (fp == NULL) {
        THROW_ERROR("failed to open host cpuid.txt");
    }
    while (fgets(buff, BUFF_SIZE, fp)) {
        uint32_t leaf = 0;
        uint32_t subleaf = 0;
        t_cpuid_t cpu = {0};
        int num = sscanf(buff, "   %x %x: eax=%x ebx=%x ecx=%x edx=%x", &leaf, &subleaf,
                         &cpu.eax, &cpu.ebx, &cpu.ecx, &cpu.edx);
        if (num != 6) {
            continue;
        }
        t_cpuid_t cpu_sgx = {0};
        native_cpuid(leaf, subleaf, &cpu_sgx);
        if (!is_cpuidinfo_equal(leaf, &cpu, &cpu_sgx)) {
            printf("leaf:0x%x subleaf:0x%x\n", leaf, subleaf);
            printf("ori_eax:0x%x ori_ebx:0x%x ori_ecx:0x%x ori_edx:0x%x\n",
                   cpu.eax, cpu.ebx, cpu.ecx, cpu.edx);
            printf("sgx_eax:0x%x sgx_ebx:0x%x sgx_ecx:0x%x sgx_edx:0x%x\n",
                   cpu_sgx.eax, cpu_sgx.ebx, cpu_sgx.ecx, cpu_sgx.edx);
            THROW_ERROR("failed to check cpuid info");
        }
    }
    fclose(fp);
    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_cpuid_with_basic_leaf_zero),
    TEST_CASE(test_cpuid_with_basic_leaf_zero_with_subleaf),
    TEST_CASE(test_cpuid_with_extend_leaf_zero),
    TEST_CASE(test_cpuid_with_extend_leaf_zero_with_subleaf),
    TEST_CASE(test_cpuid_with_basic_leaf_one),
    TEST_CASE(test_cpuid_with_sgx_verify),
    TEST_CASE(test_cpuid_with_sgx_enumeration),
    TEST_CASE(test_cpuid_with_invalid_leaf),
    TEST_CASE(test_cpuid_with_oversized_leaf),
    TEST_CASE(test_cpuid_with_random_leaf),
    TEST_CASE(test_cpuid_with_host_cpuidinfo),
};

int main() {
    g_sgx_supported = is_sgx_supported();
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
