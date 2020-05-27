#include <unistd.h>
#include <string.h>
#include <stdio.h>

int main(int argc, const char *argv[]) {
    if (argc <= 1) {
        printf("Hello World!\n");
    } else {
        const char *echo_msg = argv[1];
        printf("%s\n", echo_msg);
    }
    return 0;
}
