#define _GNU_SOURCE
#include <sched.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <assert.h>
#include <sys/time.h>
#include <sys/types.h>
#include <sys/stat.h>

#define TRUE 1
#define FALSE 0
#define PAGE_SIZE 4096

typedef struct pthread_arg_t {
    void* buf;
    double duration;
    long process_bytes;
} pthread_arg_t;

const double MB_SIZE = 1024 * 1024;
const double KB_SIZE = 1024;

int thread_num;
int file_num;
long file_block_size;
int file_req_merge_num;
long file_total_size;
int is_read;
int is_seq;
int use_fsync;
int use_direct;
int loops;

long position;
int current_file;

int* fds;
pthread_t* tid;
pthread_arg_t* pthread_args;
pthread_mutex_t lock;

void prepare();
void do_tasks();
void done();
int get_next_request(int* fd, long* offset, int* size);
void *pthread_routine(void *arg);
u_int32_t get_random();


void prepare() {
    position = 0;
    current_file = 0;

    fds = (int*)malloc(file_num * sizeof(int));

    void* buf = NULL;
    if (posix_memalign(&buf, PAGE_SIZE, file_block_size) != 0) {
        perror("posix_memalign");
        return;
    }

    char file_name[512];
    long file_size = file_total_size / file_num;
    for (int i = 0; i < file_num; ++i) {
        snprintf(file_name, sizeof(file_name), "test_file.%d", i);
        int flags = O_RDWR | O_CREAT | O_TRUNC;
        if (use_direct) flags |= O_DIRECT;
        int fd = open(file_name, flags, S_IRUSR | S_IRUSR);
        if (fd < 0) {
            perror("open");
            return;
        }
        fds[i] = fd;

        for (int j = 0; j < file_size; j += file_block_size) {
            int ret = write(fd, buf, file_block_size);
            assert(ret == file_block_size);
        }
    }
    free(buf);

    tid = (pthread_t*)malloc(thread_num * sizeof(pthread_t));
    pthread_args = (pthread_arg_t*)malloc(thread_num * sizeof(pthread_arg_t));
    for (int i = 0; i < thread_num; ++i) {
        if (posix_memalign(&pthread_args[i].buf, 4096, file_block_size) != 0) {
            perror("posix_memalign");
            return;
        };
        pthread_args[i].process_bytes = 0;
        pthread_args[i].duration = 0;
    }
}

void do_tasks() {
    pthread_attr_t pthread_attr;
    if (pthread_attr_init(&pthread_attr) != 0) {
        perror("pthread_attr_init");
        exit(1);
    }

    cpu_set_t cpus;
    CPU_ZERO(&cpus);
    CPU_SET(1, &cpus);
    // sched_setaffinity(0, sizeof(cpu_set_t), &cpus);
    pthread_attr_setaffinity_np(&pthread_attr, sizeof(cpu_set_t), &cpus);

    if (pthread_mutex_init(&lock, NULL) != 0) {
        perror("pthread_mutex_init");
        return; 
    }

    struct timeval tv1;
    gettimeofday(&tv1, NULL);
    
    for (int i = 0; i < thread_num; ++i) {
        if (pthread_create(&tid[i], &pthread_attr, pthread_routine, (void *)&pthread_args[i]) != 0) {
            perror("pthread_create");
            return;
        }
    }

    long total_process_bytes = 0;
    for (int i = 0; i < thread_num; i++) {
       pthread_join(tid[i], NULL);
       total_process_bytes += pthread_args[i].process_bytes;
    }

    struct timeval tv2;
    gettimeofday(&tv2, NULL);
    double duration = (double) (tv2.tv_usec - tv1.tv_usec) / 1000000 + (double) (tv2.tv_sec - tv1.tv_sec);

    double throughput = total_process_bytes / MB_SIZE / duration;
    printf("duration: %f s, throughput: %f MB/s\n", duration, throughput);
}

void done() {
    for (int i = 0; i < file_num; ++i) {
        close(fds[i]);
    }
    free(fds);

    for (int i = 0; i < thread_num; ++i) {
        free(pthread_args[i].buf);
    }

    free(tid);
    free(pthread_args);
    pthread_mutex_destroy(&lock); 
}

int seed = 0;
u_int32_t get_random()
{
    u_int32_t hi, lo;
    hi = (seed = seed * 1103515245 + 12345) >> 16;
    lo = (seed = seed * 1103515245 + 12345) >> 16;
    return (hi << 16) + lo;
}

int get_next_request(int* fd, long* offset, int* size) {
    pthread_mutex_lock(&lock);

    // todo

    pthread_mutex_unlock(&lock); 
}

void *pthread_routine(void *arg) {
    pthread_arg_t *pthread_arg = (pthread_arg_t *)arg;
    char* buf = (char*)pthread_arg->buf;

    struct timeval tv1;
    gettimeofday(&tv1, NULL);

    long file_size = file_total_size / file_num;
    int fd = fds[0];
    for (int i = 0; i < loops; ++i) {
        if (is_read) {
            if (is_seq) {
                for (long offset = 0; offset < file_size; offset += file_block_size) {
                    int bytes_read = pread(fd, buf, file_block_size, offset);
                    assert(bytes_read == file_block_size);
                }
            } else {
                long cnt = 0;
                int block_num = file_size / file_block_size;
                while (cnt < file_size) {
                    int offset = (get_random() % block_num) * file_block_size;
                    int bytes_read = pread(fd, buf, file_block_size, offset);
                    assert(bytes_read == file_block_size);
                    cnt += bytes_read;
                }
            }
        }
        else {
            if (is_seq) {
                for (long offset = 0; offset < file_size; offset += file_block_size) {
                    int bytes_write = pwrite(fd, buf, file_block_size, offset);
                    assert(bytes_write == file_block_size);
                }
            }
            else {
                long cnt = 0;
                int block_num = file_size / file_block_size;
                while (cnt < file_size) {
                    int offset = (get_random() % block_num) * file_block_size;
                    int bytes_write = pwrite(fd, buf, file_block_size, offset);
                    assert(bytes_write == file_block_size);
                    cnt += bytes_write;
                }
            }

            if (use_fsync) fsync(fd);
        }
    }

    struct timeval tv2;
    gettimeofday(&tv2, NULL);
    pthread_arg->duration = (double) (tv2.tv_usec - tv1.tv_usec) / 1000000 + (double) (tv2.tv_sec - tv1.tv_sec);
    pthread_arg->process_bytes = file_size * loops;
    return NULL;
}

int main(int argc, char *argv[]) {
    thread_num = argc > 1 ? atoi(argv[1]) : 1;
    file_num = argc > 2 ? atoi(argv[2]) : 1;
    file_block_size = argc > 3 ? atoi(argv[3]) : 4; // KB
    file_block_size *= KB_SIZE;
    file_req_merge_num = argc > 4 ? atoi(argv[4]) : 10;
    file_total_size = argc > 5 ? atoi(argv[5]) : 100; // MB
    file_total_size *= MB_SIZE;

    is_read = argc > 6 ? atoi(argv[6]) : 1;
    is_seq = argc > 7 ? atoi(argv[7]) : 1;
    use_fsync = argc > 8 ? atoi(argv[8]) : 1;
    use_direct = argc > 9 ? atoi(argv[9]) : 1;
    loops = argc > 10 ? atoi(argv[10]) : 1;

    printf("[thread_num: %d, file_num: %d, file_block_size: %ld, file_req_merge_num: %d, file_total_size: %ld, ",
        thread_num, file_num, file_block_size, file_req_merge_num, file_total_size);
    printf("is_read: %d, is_seq: %d, use_fsync: %d, use_direct: %d, loop: %d] ",
        is_read, is_seq, use_fsync, use_direct, loops);

    prepare();

    do_tasks();

    done();
    
    return 0;
}