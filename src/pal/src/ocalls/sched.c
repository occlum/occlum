#define _GNU_SOURCE
#include <sched.h>
#include <dirent.h>
#include <unistd.h>
#include "ocalls.h"

static int is_number(const char *str) {
    size_t len = strlen(str);
    for (size_t i = 0; i < len; i++) {
        if (str[i] >= '0' && str[i] <= '9') {
            continue;
        }
        return 0;
    }
    return len > 0;
}

static int is_node_entry(struct dirent *d) {
    return
        d &&
        strncmp(d->d_name, "node", 4) == 0 &&
        is_number(d->d_name + 4);
}

// The information about NUMA topology is stored in sysfs.
// By reading the node entries(node<id>) in /sys/devices/system/cpu/cpu<id>,
// we can find which cpu core locates at which NUMA node.
int occlum_ocall_get_numa_topology(uint32_t *numa_buf, size_t ncpus) {
    uint32_t *ptr = numa_buf;
    for (size_t i = 0; i < ncpus; i++) {
        struct dirent *d;
        char cpu_dir_path[128] = { 0 };
        int ret = snprintf(cpu_dir_path, sizeof(cpu_dir_path), "/sys/devices/system/cpu/cpu%ld",
                           i);
        if (ret < 0 || ret >= sizeof(cpu_dir_path)) {
            return -1;
        }
        DIR *dir = opendir(cpu_dir_path);
        if (dir == NULL) {
            return -1;
        }
        while ((d = readdir(dir))) {
            if (is_node_entry(d)) {
                errno = 0;
                int node_id = strtol((d->d_name) + 4, (char **)NULL, 10);
                if (errno) {
                    closedir(dir);
                    return -1;
                }
                *ptr = node_id;
                break;
            }
        }
        closedir(dir);
        ptr++;
    }
    return 0;
}
