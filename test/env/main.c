#include <string.h>
#include <stdio.h>
#include <elf.h>
#include <errno.h>
#include <sys/auxv.h>
#include <stdlib.h>

// Expected arguments are given by Makefile throught macro ARGC, ARG1, ARG2 and
// ARG3
const char* expected_argv[EXPECTED_ARGC] = {
    "env",
    EXPECTED_ARG1,
    EXPECTED_ARG2,
    EXPECTED_ARG3,
};

int main(int argc, const char* argv[]) {
    // Test argc
    if (argc != EXPECTED_ARGC) {
        printf("ERROR: expect %d arguments, but %d are given\n", EXPECTED_ARGC, argc);
        return -1;
    }

    // Test argv
    for (int arg_i = 0; arg_i < argc; arg_i++) {
        printf("arg[%d] = %s\n", arg_i, argv[arg_i]);
        const char* actual_arg = argv[arg_i];
        const char* expected_arg = expected_argv[arg_i];
        if (strcmp(actual_arg, expected_arg) != 0) {
            printf("ERROR: expect argument %d is %s, but given %s\n",
                    arg_i, expected_arg, actual_arg);
            return -1;
        }
    }

    // Test envp
    // Occlum LibOS by default passes an environment OCCLUM=yes
    const char* env_val = getenv("OCCLUM");
    if (env_val == 0) {
        printf("ERROR: cannot find environment variable OCCLUM\n");
        return -1;
    }
    else if (strcmp(env_val, "yes") != 0) {
        printf("ERROR: environment variable OCCLUM=yes expected, but given %s\n",
                env_val);
        return -1;
    }

    // Test aux
    unsigned long page_size = getauxval(AT_PAGESZ);
    if (errno != 0 || page_size != 4096) {
        printf("ERROR: auxilary vector does not pass correct the value\n");
        return -1;
    }

    return 0;
}
