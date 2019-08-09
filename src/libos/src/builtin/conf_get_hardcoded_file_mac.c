#include <stddef.h>

// The 128-bit MAC of Occlum.json
// Should be provided by Makefile; Set it to all zeros by default.
#ifndef OCCLUM_BUILTIN_CONF_FILE_MAC
#define ALL_ZEROS_32BIT                 "00-00-00-00"
#define ALL_ZEROS_128BIT                (ALL_ZEROS_32BIT"-"ALL_ZEROS_32BIT"-"\
                                         ALL_ZEROS_32BIT"-"ALL_ZEROS_32BIT)
#define OCCLUM_BUILTIN_CONF_FILE_MAC    ALL_ZEROS_128BIT
#endif

const char* conf_get_hardcoded_file_mac(void) {
    return OCCLUM_BUILTIN_CONF_FILE_MAC;
}
