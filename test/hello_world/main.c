#include <unistd.h>
#include <string.h>

static const char* msg = "Hello World\n";

int main(void) {
    write(1, msg, strlen(msg) + 1);
    return 0;
}
