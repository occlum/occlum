#include <unistd.h>
#include <string.h>
#include <stdio.h>

static const char* msg = "Hello World\n";

int main(int argc) {
    printf("%s", msg);
    return 0;
}
