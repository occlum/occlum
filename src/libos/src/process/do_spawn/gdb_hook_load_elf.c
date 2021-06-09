#include <sys/types.h>

/*
 * Providing debug symbol information to GDB.
 *
 * This function is left empty deliberately and should NOT be removed.
 *
 * When SGX GDB is attached to the enclave, a break point will be inserted at the beginning
 * of this function. So when this function is called at runtime, GDB can capture the arguments
 * of this function, which gives the name of a loaded ELF file and the memory location where
 * the ELF is loaded in the enclave. With this information, GDB can translate memory addresses
 * to symbol names, thus give meaningful debug information.
 *
 * `__attribute__((optimize("O0")))` is used to prevent the compiler from optimizing this function in
 * unexpected way (e.g., eliminating this empty function).
 */
void __attribute__((optimize("O0"))) occlum_gdb_hook_load_elf(
    uint64_t elf_base,
    const char *elf_path,
    uint64_t elf_path_len) {
}
