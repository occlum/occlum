#include <unistd.h>
#include "ocalls.h"

void occlum_ocall_sync(void) {
    sync();
}
