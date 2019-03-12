#include <string.h>
#include <stdio.h>

// Expected arguments are given by Makefile throught macro ARGC, ARG1, ARG2 and
// ARG3
const char* expected_argv[EXPECTED_ARGC] = {
    "argv",
    EXPECTED_ARG1,
    EXPECTED_ARG2,
    EXPECTED_ARG3,
};

int main(int argc, const char* argv[]) {
    if (argc != EXPECTED_ARGC) {
        printf("ERROR: expect %d arguments, but %d are given\n", EXPECTED_ARGC, argc);
        return -1;
    }

    for (int arg_i = 0; arg_i < argc; arg_i++) {
        const char* actual_arg = argv[arg_i];
        const char* expected_arg = expected_argv[arg_i];
        if (strcmp(actual_arg, expected_arg) != 0) {
            printf("ERROR: expect argument %d is %s, but given %s\n",
                    arg_i, expected_arg, actual_arg);
        }
    }

    printf("main()'s argc and argv are as expected\n");
    return 0;
}
