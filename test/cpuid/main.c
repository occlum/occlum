#include <stdio.h>
#include <stdint.h>
#include <string.h>

typedef struct t_cpuid {
    unsigned int eax;
    unsigned int ebx;
    unsigned int ecx;
    unsigned int edx;
} t_cpuid_t;

static inline void native_cpuid(int leaf, int subleaf, t_cpuid_t *p)
{
    memset(p, 0, sizeof(*p));
    /* ecx is often an input as well as an output. */
    asm volatile("cpuid"
        : "=a" (p->eax),
          "=b" (p->ebx),
          "=c" (p->ecx),
          "=d" (p->edx)
        : "a" (leaf), "c" (subleaf));
}

int main(int argc, char **argv)
{
    /* Gets CPUID information and tests the SGX support of the CPU */
    t_cpuid_t cpu;
    int leaf = 1;
    int subleaf = 0;

    native_cpuid(leaf, subleaf, &cpu);
    printf("eax: %x ebx: %x ecx: %x edx: %x\n", cpu.eax, cpu.ebx, cpu.ecx, cpu.edx);
    printf("Stepping %d\n", cpu.eax & 0xF); // Bit 3-0
    printf("Model %d\n", (cpu.eax >> 4) & 0xF); // Bit 7-4
    printf("Family %d\n", (cpu.eax >> 8) & 0xF); // Bit 11-8
    printf("Processor Type %d\n", (cpu.eax >> 12) & 0x3); // Bit 13-12
    printf("Extended Model %d\n", (cpu.eax >> 16) & 0xF); // Bit 19-16
    printf("Extended Family %d\n", (cpu.eax >> 20) & 0xFF); // Bit 27-20

    // if smx (Safer Mode Extensions) set - SGX global enable is supported
    printf("smx: %d\n", (cpu.ecx >> 6) & 1); // CPUID.1:ECX.[bit6]

    /* Extended feature bits (EAX=07H, ECX=0H)*/
    printf("\nExtended feature bits (EAX=07H, ECX=0H)\n");
    leaf = 7;
    subleaf = 0;
    native_cpuid(leaf, subleaf, &cpu);
    printf("eax: %x ebx: %x ecx: %x edx: %x\n", cpu.eax, cpu.ebx, cpu.ecx, cpu.edx);

    //CPUID.(EAX=07H, ECX=0H):EBX.SGX = 1,
    // Bit 02: SGX. Supports Intel® Software Guard Extensions (Intel® SGX Extensions) if 1.
    printf("SGX is available: %d\n", (cpu.ebx >> 2) & 0x1);

    /* SGX has to be enabled in MSR.IA32_Feature_Control.SGX_Enable
       check with msr-tools: rdmsr -ax 0x3a
       SGX_Enable is Bit 18
       if SGX_Enable = 0 no leaf information will appear.
       for more information check Intel Docs Architectures-software-developer-system-programming-manual - 35.1 Architectural MSRS
    */

    /* CPUID Leaf 12H, Sub-Leaf 0 Enumeration of Intel SGX Capabilities (EAX=12H,ECX=0) */
    printf("\nCPUID Leaf 12H, Sub-Leaf 0 of Intel SGX Capabilities (EAX=12H,ECX=0)\n");
    leaf = 0x12;
    subleaf = 0;
    native_cpuid(leaf, subleaf, &cpu);
    printf("eax: %x ebx: %x ecx: %x edx: %x\n", cpu.eax, cpu.ebx, cpu.ecx, cpu.edx);

    printf("Sgx 1 supported: %d\n", cpu.eax & 0x1);
    printf("Sgx 2 supported: %d\n", (cpu.eax >> 1) & 0x1);
    printf("MaxEnclaveSize_Not64: %x\n", cpu.edx & 0xFF);
    printf("MaxEnclaveSize_64: %x\n", (cpu.edx >> 8) & 0xFF);

    /* CPUID Leaf 12H, Sub-Leaf 1 Enumeration of Intel SGX Capabilities (EAX=12H,ECX=1) */
    printf("\nCPUID Leaf 12H, Sub-Leaf 1 of Intel SGX Capabilities (EAX=12H,ECX=1)\n");
    leaf = 0x12;
    subleaf = 1;
    native_cpuid(leaf, subleaf, &cpu);
    printf("eax: %x ebx: %x ecx: %x edx: %x\n", cpu.eax, cpu.ebx, cpu.ecx, cpu.edx);

    int i;
    for (i=2; i<4; i++) {
      /* CPUID Leaf 12H, Sub-Leaf i Enumeration of Intel SGX Capabilities (EAX=12H,ECX=i) */
      printf("\nCPUID Leaf 12H, Sub-Leaf %d of Intel SGX Capabilities (EAX=12H,ECX=%d)\n", i, i);
      leaf = 0x12;
      subleaf = i;
      native_cpuid(leaf, subleaf, &cpu);
      printf("eax: %x ebx: %x ecx: %x edx: %x\n", cpu.eax, cpu.ebx, cpu.ecx, cpu.edx);
    }

    return 0;
}
