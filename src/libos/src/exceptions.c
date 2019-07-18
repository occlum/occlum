#include <stdlib.h>
#include <stdbool.h>

#include "sgx_cpuid.h"
#include "sgx_trts_exception.h"

#define CPUID_OPCODE 0xA20F
#define RDTSC_OPCODE 0x310F
#define SUPPORTED_CPUID_LEAF_NUM 30
// the maximum supported sub-leaves may vary between different leaves and
// processors, fix it to a constant for now
#define SUPPORTED_CPUID_SUBLEAF_NUM 4

int supported_cpuid_leaves[] = {
    // Basic CPUID Information
    0x00000000, 0x00000001, 0x00000002, 0x00000003, 0x00000004, 0x00000005,
    0x00000006, 0x00000007, 0x00000009, 0x0000000A, 0x0000000B, 0x0000000D,
    0x0000000F, 0x00000010, 0x00000012, 0x00000014, 0x00000015, 0x00000016,
    0x00000017, 0x00000018, 0x0000001F,
    // Extended Function CPUID Information
    0x80000000, 0x80000001, 0x80000002, 0x80000003, 0x80000004, 0x80000005,
    0x80000006, 0x80000007, 0x80000008,
};

// holds cached CPUID information
typedef struct _CpuidInfo {
    int leaf;
    int subleaf;
    int reg[4];
} CpuidInfo;
CpuidInfo cpuid_info[SUPPORTED_CPUID_LEAF_NUM][SUPPORTED_CPUID_SUBLEAF_NUM];

// rdtsc support here is temporary, only for SKL, later CPU's will support this inside enclave
uint64_t fake_rdtsc_value = 0;
uint16_t fake_rdtsc_inc_value = 1000;

void setup_cpuid_info() {
    for (int i = 0; i < SUPPORTED_CPUID_LEAF_NUM; i++) {
        for (int j = 0; j < SUPPORTED_CPUID_SUBLEAF_NUM; j++) {
            int index = supported_cpuid_leaves[i];
            cpuid_info[i][j].leaf = index;
            cpuid_info[i][j].subleaf = j;
            if (sgx_cpuidex(cpuid_info[i][j].reg, index, j) != SGX_SUCCESS)
                abort();
        }
    }
}

int handle_cpuid_exception(sgx_exception_info_t *info) {
    uint16_t ip_opcode = *(uint16_t *)(info->cpu_context.rip);
    uint64_t leaf;
    uint64_t subleaf;

    if (info->exception_vector != SGX_EXCEPTION_VECTOR_UD ||
        info->exception_type != SGX_EXCEPTION_HARDWARE ||
        ip_opcode != CPUID_OPCODE) {
        return EXCEPTION_CONTINUE_SEARCH;
    }

    leaf = info->cpu_context.rax;
    subleaf = info->cpu_context.rcx;

    for (int i = 0; i < SUPPORTED_CPUID_LEAF_NUM; i++) {
        for (int j = 0; j < SUPPORTED_CPUID_SUBLEAF_NUM; j++) {
            if (cpuid_info[i][j].leaf == leaf &&
                cpuid_info[i][j].subleaf == subleaf) {
                info->cpu_context.rax = cpuid_info[i][j].reg[0];
                info->cpu_context.rbx = cpuid_info[i][j].reg[1];
                info->cpu_context.rcx = cpuid_info[i][j].reg[2];
                info->cpu_context.rdx = cpuid_info[i][j].reg[3];

                info->cpu_context.rip += 2;
                return EXCEPTION_CONTINUE_EXECUTION;
            }
        }
    }
    return EXCEPTION_CONTINUE_SEARCH;
}

int handle_rdtsc_exception(sgx_exception_info_t *info) {
    uint16_t ip_opcode = *(uint16_t *)(info->cpu_context.rip);

    if (info->exception_vector != SGX_EXCEPTION_VECTOR_UD ||
        info->exception_type != SGX_EXCEPTION_HARDWARE ||
        ip_opcode != RDTSC_OPCODE) {
        return EXCEPTION_CONTINUE_SEARCH;
    }

    fake_rdtsc_value += fake_rdtsc_inc_value;
    info->cpu_context.rax = (uint32_t)(fake_rdtsc_value & 0xFFFFFFFF);
    info->cpu_context.rdx = (uint32_t)(fake_rdtsc_value >> 32);
    info->cpu_context.rip += 2;

    return EXCEPTION_CONTINUE_EXECUTION;
}

void register_exception_handlers() {
    setup_cpuid_info();
    sgx_register_exception_handler(true, handle_cpuid_exception);
    sgx_register_exception_handler(true, handle_rdtsc_exception);
}
