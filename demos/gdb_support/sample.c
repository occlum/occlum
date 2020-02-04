#include <stdio.h>

// GDB will stop here
int divide_by_zero(int num) {
    int result;
    int zero = 0;

    result = num / zero;
    return result;
}

int main() {
    int ret;

    printf("Start to calculate\n");
    ret = divide_by_zero(1);
    return 0;
}
