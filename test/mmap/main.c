#define _GNU_SOURCE
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>
#include <errno.h>
#include <assert.h>
#include <string.h>
#include <fcntl.h>
#include <sys/syscall.h>
#include "test_fs.h"

// ============================================================================
// Helper macros
// ============================================================================

#define KB                      (1024UL)
#define MB                      (1024 * 1024UL)
#define PAGE_SIZE               (4 * KB)

#define ALIGN_DOWN(x, a)        ((x) & ~(a-1)) // a must be a power of two
#define ALIGN_UP(x, a)          ALIGN_DOWN((x+(a-1)), (a))

#define MAX_MMAP_USED_MEMORY    (4 * MB)

// ============================================================================
// Helper functions
// ============================================================================

static void *get_a_stack_ptr() {
    volatile int a = 0;
    return (void *) &a;
}

// ============================================================================
// Test suite initialization
// ============================================================================

// Get a valid range of address hints for mmap
static int get_a_valid_range_of_hints(size_t *hint_begin, size_t *hint_end) {
    size_t big_buf_len = MAX_MMAP_USED_MEMORY;
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    void *big_buf = mmap(NULL, big_buf_len, prot, flags, -1, 0);
    if (big_buf == MAP_FAILED) {
        THROW_ERROR("mmap failed");
    }

    // Check if munmap will clean the range
    memset(big_buf, 0xff, big_buf_len);

    int ret = munmap(big_buf, big_buf_len);
    if (ret < 0) {
        THROW_ERROR("munmap failed");
    }
    *hint_begin = (size_t)big_buf;
    *hint_end = *hint_begin + big_buf_len;
    return 0;
}

static size_t HINT_BEGIN, HINT_END;

int test_suite_init() {
    if (get_a_valid_range_of_hints(&HINT_BEGIN, &HINT_END) < 0) {
        THROW_ERROR("get_a_valid_range_of_hints failed");
    }
    return 0;
}

// ============================================================================
// Test cases for anonymous mmap
// ============================================================================

int test_anonymous_mmap() {
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;

    for (size_t len = PAGE_SIZE; len <= MAX_MMAP_USED_MEMORY; len *= 2) {
        void *buf = mmap(NULL, len, prot, flags, -1, 0);
        if (buf == MAP_FAILED) {
            THROW_ERROR("mmap failed");
        }

        if (check_bytes_in_buf(buf, len, 0) < 0) {
            THROW_ERROR("the buffer is not initialized to zeros");
        }

        int ret = munmap(buf, len);
        if (ret < 0) {
            THROW_ERROR("munmap failed");
        }
    }
    return 0;
}

int test_anonymous_mmap_randomly() {
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;

    void *bufs[16] = {NULL};
    size_t lens[16];
    size_t num_bufs = 0;
    size_t used_memory = 0;

    for (int i = 0; i < 5; i++) {
        // Phrase 1: do mmap with random sizes until no more buffers or memory
        for (num_bufs = 0;
                num_bufs < ARRAY_SIZE(bufs) && used_memory < MAX_MMAP_USED_MEMORY;
                num_bufs++) {
            // Choose the mmap size randomly
            size_t len = rand() % (MAX_MMAP_USED_MEMORY - used_memory) + 1;
            len = ALIGN_UP(len, PAGE_SIZE);

            // Do mmap
            void *buf = mmap(NULL, len, prot, flags, -1, 0);
            if (buf == MAP_FAILED) {
                THROW_ERROR("mmap failed");
            }
            bufs[num_bufs] = buf;
            lens[num_bufs] = len;

            // Update memory usage
            used_memory += len;
        }

        // Phrase 2: do munmap to free all memory mapped memory
        for (int bi = 0; bi < num_bufs; bi++) {
            void *buf = bufs[bi];
            size_t len = lens[bi];
            int ret = munmap(buf, len);
            if (ret < 0) {
                THROW_ERROR("munmap failed");
            }

            bufs[bi] = NULL;
            lens[bi] = 0;
        }

        num_bufs = 0;
        used_memory = 0;
    }

    return 0;
}

int test_anonymous_mmap_randomly_with_good_hints() {
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    for (int i = 0; i < 10; i++) {
        size_t hint = HINT_BEGIN + rand() % (HINT_END - HINT_BEGIN);
        hint = ALIGN_DOWN(hint, PAGE_SIZE);

        size_t len = rand() % (HINT_END - (size_t)hint);
        len = ALIGN_UP(len + 1, PAGE_SIZE);

        void *addr = mmap((void *)hint, len, prot, flags, -1, 0);
        if (addr != (void *)hint) {
            THROW_ERROR("mmap with hint failed");
        }

        int ret = munmap(addr, len);
        if (ret < 0) {
            THROW_ERROR("munmap failed");
        }
    }
    return 0;
}

int test_anonymous_mmap_with_bad_hints() {
    size_t bad_hints[] = {
        PAGE_SIZE, // too low!
        0xffff800000000000UL, // too high!
        ALIGN_DOWN((size_t)get_a_stack_ptr(), PAGE_SIZE), // overlapped with stack!
        HINT_BEGIN + 123, // within the valid range, not page aligned!
    };
    int len = PAGE_SIZE;
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    for (int hi = 0; hi < ARRAY_SIZE(bad_hints); hi++) {
        void *bad_hint = (void *)bad_hints[hi];
        void *addr = mmap(bad_hint, len, prot, flags, -1, 0);
        if (addr == MAP_FAILED) {
            THROW_ERROR("mmap should have tolerated a bad hint");
        }
        if (addr == bad_hint) {
            THROW_ERROR("mmap should not have accepted a bad hint");
        }
        int ret = munmap(addr, len);
        if (ret < 0) {
            THROW_ERROR("munmap failed");
        }
    }
    return 0;
}

int test_anonymous_mmap_with_zero_len() {
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    int len = 0; // invalid!
    void *buf = mmap(NULL, len, prot, flags, -1, 0);
    if (buf != MAP_FAILED) {
        THROW_ERROR("mmap with zero len should have been failed");
    }
    return 0;
}

int test_anonymous_mmap_with_non_page_aligned_len() {
    int len = PAGE_SIZE + 17; // length need not to be page aligned!
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    void *buf = mmap(NULL, len, prot, flags, -1, 0);
    if (buf == MAP_FAILED) {
        THROW_ERROR("mmap with non-page aligned len should have worked");
    }

    // Even the length is not page aligned, the page mmaping is done in pages
    if (check_bytes_in_buf(buf, ALIGN_UP(len, PAGE_SIZE), 0) < 0) {
        THROW_ERROR("the buffer is not initialized to zeros");
    }

    int ret = munmap(buf, len);
    if (ret < 0) {
        THROW_ERROR("munmap failed");
    }
    return 0;
}

// ============================================================================
// Test cases for file-backed mmap
// ============================================================================

int test_private_file_mmap() {
    const char *file_path = "/root/mmap_file.data";
    int fd = open(file_path, O_CREAT | O_TRUNC | O_WRONLY, 0644);
    if (fd < 0) {
        THROW_ERROR("file creation failed");
    }
    int file_len = 12 * KB + 128;
    int byte_val = 0xab;
    fill_file_with_repeated_bytes(fd, file_len, byte_val);
    close(fd);

    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE;
    fd = open(file_path, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("file open failed");
    }
    off_t offset = 0;
    for (size_t len = PAGE_SIZE; len <= file_len; len *= 2) {
        char *buf = mmap(NULL, len, prot, flags, fd, offset);
        if (buf == MAP_FAILED) {
            THROW_ERROR("mmap failed");
        }

        if (check_bytes_in_buf(buf, len, byte_val) < 0) {
            THROW_ERROR("the buffer is not initialized according to the file");
        }

        int ret = munmap(buf, len);
        if (ret < 0) {
            THROW_ERROR("munmap failed");
        }
    }
    close(fd);
    unlink(file_path);
    return 0;
}

int test_private_file_mmap_with_offset() {
    const char *file_path = "/root/mmap_file.data";
    int fd = open(file_path, O_CREAT | O_TRUNC | O_RDWR, 0644);
    if (fd < 0) {
        THROW_ERROR("file creation failed");
    }
    size_t first_len = 4 * KB + 47;
    int first_val = 0xab;
    fill_file_with_repeated_bytes(fd, first_len, first_val);
    size_t second_len = 9 * KB - 47;
    int second_val = 0xcd;
    fill_file_with_repeated_bytes(fd, second_len, second_val);
    size_t file_len = first_len + second_len;

    off_t offset = 4 * KB;
    int len = file_len - offset + 1 * KB;
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE;
    assert(offset <= first_len);
    char *buf = mmap(NULL, len, prot, flags, fd, offset);
    if (buf == MAP_FAILED) {
        THROW_ERROR("mmap failed");
    }

    char *buf_cursor = buf;
    if (check_bytes_in_buf(buf_cursor, first_len - offset, first_val) < 0) {
        THROW_ERROR("the buffer is not initialized according to the file");
    }
    buf_cursor += first_len - offset;
    if (check_bytes_in_buf(buf_cursor, second_len, second_val) < 0) {
        THROW_ERROR("the buffer is not initialized according to the file");
    }
    buf_cursor += second_len;
    if (check_bytes_in_buf(buf_cursor, ALIGN_UP(len, PAGE_SIZE) - (buf_cursor - buf),
                           0) < 0) {
        THROW_ERROR("the remaining of the last page occupied by the buffer is not initialized to zeros");
    }

    int ret = munmap(buf, len);
    if (ret < 0) {
        THROW_ERROR("munmap failed");
    }

    close(fd);
    unlink(file_path);
    return 0;
}

int test_private_file_mmap_with_invalid_fd() {
    size_t len = PAGE_SIZE;
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE;
    int fd = 1234; // invalid!
    off_t offset = 0;
    void *buf = mmap(NULL, len, prot, flags, fd, offset);
    if (buf != MAP_FAILED) {
        THROW_ERROR("file mmap with an invalid fd should have been failed");
    }
    return 0;
}

int test_private_file_mmap_with_non_page_aligned_offset() {
    const char *file_path = "/root/mmap_file.data";
    int fd = open(file_path, O_CREAT | O_TRUNC | O_RDWR, 0644);
    if (fd < 0) {
        THROW_ERROR("file creation failed");
    }
    int file_len = 12 * KB + 128;
    int byte_val = 0xab;
    fill_file_with_repeated_bytes(fd, file_len, byte_val);

    size_t len = PAGE_SIZE;
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    off_t  offset = PAGE_SIZE + 127; // Invalid!
    void *buf = mmap(NULL, len, prot, flags, fd, offset);
    if (buf != MAP_FAILED) {
        THROW_ERROR("mmap with non-page aligned len should have been failed");
    }

    close(fd);
    unlink(file_path);
    return 0;
}

// TODO: what if offset > file size or offset + len > file size?


typedef int (*flush_file_mmap_func_t)(int /*fd*/, void * /*addr*/, size_t /*size*/);

static int __test_shared_file_mmap_flushing_file(flush_file_mmap_func_t flush_fn) {
    // Update a file by writing to its file-backed memory mapping
    const char *file_path = "/root/mmap_file.data";
    int fd = open(file_path, O_CREAT | O_TRUNC | O_RDWR, 0644);
    if (fd < 0) {
        THROW_ERROR("file creation failed");
    }
    if (fill_file_with_repeated_bytes(fd, PAGE_SIZE, 0) < 0) {
        THROW_ERROR("file init failed");
    }

    int byte_val = 0xde;
    char *write_buf = mmap(NULL, PAGE_SIZE, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (write_buf == MAP_FAILED) {
        THROW_ERROR("mmap failed");
    }
    for (int i = 0; i < PAGE_SIZE; i++) { write_buf[i] = byte_val; }

    int ret = flush_fn(fd, write_buf, PAGE_SIZE);
    if (ret < 0) {
        THROW_ERROR("fdatasync failed");
    }
    close(fd);

    // Read the file back to see if the updates are durable
    fd = open(file_path, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("file open failed");
    }
    if (check_file_with_repeated_bytes(fd, PAGE_SIZE, byte_val) < 0) {
        THROW_ERROR("unexpected file content");
    }
    close(fd);

    unlink(file_path);
    return 0;
}

static int flush_shared_file_mmap_with_msync(int _fd, void *addr, size_t size) {
    return msync(addr, size, MS_SYNC);
}

static int flush_shared_file_mmap_with_munmap(int _fd, void *addr, size_t size) {
    return munmap(addr, size);
}

static int flush_shared_file_mmap_with_fdatasync(int fd, void *_addr, size_t _size) {
    return fsync(fd);
}

static int flush_shared_file_mmap_with_fsync(int fd, void *_addr, size_t _size) {
    return fdatasync(fd);
}

int test_shared_file_mmap_flushing_with_msync(void) {
    if (__test_shared_file_mmap_flushing_file(flush_shared_file_mmap_with_msync)) {
        THROW_ERROR("unexpected file content");
    }
    return 0;
}

int test_shared_file_mmap_flushing_with_munmap(void) {
    if (__test_shared_file_mmap_flushing_file(flush_shared_file_mmap_with_munmap)) {
        THROW_ERROR("unexpected file content");
    }
    return 0;
}

int test_shared_file_mmap_flushing_with_fdatasync(void) {
    if (__test_shared_file_mmap_flushing_file(flush_shared_file_mmap_with_fdatasync)) {
        THROW_ERROR("unexpected file content");
    }
    return 0;
}

int test_shared_file_mmap_flushing_with_fsync(void) {
    if (__test_shared_file_mmap_flushing_file(flush_shared_file_mmap_with_fsync)) {
        THROW_ERROR("unexpected file content");
    }
    return 0;
}

// ============================================================================
// Test cases for fixed mmap
// ============================================================================

int test_fixed_mmap_that_does_not_override_any_mmaping() {
    size_t hint = HINT_BEGIN + (HINT_END - HINT_BEGIN) / 3;
    hint = ALIGN_DOWN(hint, PAGE_SIZE);
    size_t len = (HINT_END - HINT_BEGIN) / 3;
    len = ALIGN_UP(len, PAGE_SIZE);
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED;
    void *addr = mmap((void *)hint, len, prot, flags, -1, 0);
    if (addr != (void *)hint) {
        THROW_ERROR("mmap with fixed address failed");
    }

    int ret = munmap(addr, len);
    if (ret < 0) {
        THROW_ERROR("munmap failed");
    }

    return 0;
}

int test_fixed_mmap_that_overrides_existing_mmaping() {
    // We're about to allocate two buffers: parent_buf and child_buf.
    // The child_buf will override a range of memory that has already
    // been allocated to the parent_buf.
    size_t parent_len = 10 * PAGE_SIZE;
    size_t pre_child_len = 2 * PAGE_SIZE, post_child_len = 3 * PAGE_SIZE;
    size_t child_len = parent_len - pre_child_len - post_child_len;

    // Allocate parent_buf
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    void *parent_buf = mmap(NULL, parent_len, prot, flags, -1, 0);
    if (parent_buf == MAP_FAILED) {
        THROW_ERROR("mmap for parent failed");
    }
    int parent_val = 0xab;
    memset(parent_buf, parent_val, parent_len);

    // Allocate child_buf
    void *child_buf = (char *)parent_buf + pre_child_len;
    if (mmap(child_buf, child_len, prot, flags | MAP_FIXED, -1, 0) != child_buf) {
        THROW_ERROR("mmap with fixed address failed");
    }

    // Check that child_buf, which overrides parent_buf, is initialized to zeros
    if (check_bytes_in_buf(child_buf, child_len, 0) < 0) {
        THROW_ERROR("the content of child mmap memory is not initialized");
    }
    // Check that the rest of parent_buf are kept intact
    if (check_bytes_in_buf((char *)child_buf - pre_child_len,
                           pre_child_len, parent_val) < 0 ||
            check_bytes_in_buf((char *)child_buf + child_len,
                               post_child_len, parent_val) < 0) {
        THROW_ERROR("the content of parent mmap memory is broken");
    }

    // Deallocate parent_buf along with child_buf
    int ret = munmap(parent_buf, parent_len);
    if (ret < 0) {
        THROW_ERROR("munmap failed");
    }

    return 0;
}

int test_fixed_mmap_with_non_page_aligned_addr() {
    size_t hint = HINT_BEGIN + 123; // Not aligned!
    size_t len = 1 * PAGE_SIZE;
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED;
    void *addr = mmap((void *)hint, len, prot, flags, -1, 0);
    if (addr != MAP_FAILED) {
        THROW_ERROR("fixed mmap with non-page aligned hint should have failed");
    }
    return 0;
}

// ============================================================================
// Test cases for munmap
// ============================================================================

static int check_buf_is_munmapped(void *target_addr, size_t len) {
    // The trivial case of zero-len meory region is considered as unmapped
    if (len == 0) { return 0; }

    // If the target_addr is not already mmaped, it should succeed to use it as
    // a hint for mmap.
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    void *real_addr = mmap(target_addr, len, prot, flags, -1, 0);
    if (real_addr != target_addr) {
        THROW_ERROR("address is already mmaped");
    }
    munmap(target_addr, len);
    return 0;
}

static int mmap_then_munmap(size_t mmap_len, ssize_t munmap_offset, size_t munmap_len) {
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED;
    // Make sure that we are manipulating memory between [HINT_BEGIN, HINT_END)
    void *mmap_addr = (void *)(munmap_offset >= 0 ? HINT_BEGIN : HINT_BEGIN - munmap_offset);
    if (mmap(mmap_addr, mmap_len, prot, flags, -1, 0) != mmap_addr) {
        THROW_ERROR("mmap failed");
    }

    void *munmap_addr = (char *)mmap_addr + munmap_offset;
    if (munmap(munmap_addr, munmap_len) < 0) {
        THROW_ERROR("munmap failed");
    }
    if (check_buf_is_munmapped(munmap_addr, munmap_len) < 0) {
        THROW_ERROR("munmap does not really free the memory");
    }

    // Make sure that when this function returns, there are no memory mappings
    // within [HINT_BEGIN, HINT_END)
    if (munmap((void *)HINT_BEGIN, HINT_END - HINT_BEGIN) < 0) {
        THROW_ERROR("munmap failed");
    }
    return 0;
}

int test_munmap_whose_range_is_a_subset_of_a_mmap_region() {
    size_t mmap_len = 4 * PAGE_SIZE;
    ssize_t munmap_offset = 1 * PAGE_SIZE;
    size_t munmap_len = 2 * PAGE_SIZE;
    if (mmap_then_munmap(mmap_len, munmap_offset, munmap_len) < 0) {
        THROW_ERROR("first mmap and then munmap failed");
    }
    return 0;
}

int test_munmap_whose_range_is_a_superset_of_a_mmap_region() {
    size_t mmap_len = 4 * PAGE_SIZE;
    ssize_t munmap_offset = -2 * PAGE_SIZE;
    size_t munmap_len = 7 * PAGE_SIZE;
    if (mmap_then_munmap(mmap_len, munmap_offset, munmap_len) < 0) {
        THROW_ERROR("first mmap and then munmap failed");
    }
    return 0;
}

int test_munmap_whose_range_intersects_with_a_mmap_region() {
    size_t mmap_len = 200 * PAGE_SIZE;
    size_t munmap_offset = 100 * PAGE_SIZE + 10 * PAGE_SIZE;
    size_t munmap_len = 4 * PAGE_SIZE;
    if (mmap_then_munmap(mmap_len, munmap_offset, munmap_len) < 0) {
        THROW_ERROR("first mmap and then munmap failed");
    }
    return 0;
}

int test_munmap_whose_range_intersects_with_no_mmap_regions() {
    size_t mmap_len = 1 * PAGE_SIZE;
    size_t munmap_offset = 1 * PAGE_SIZE;
    size_t munmap_len = 1 * PAGE_SIZE;
    if (mmap_then_munmap(mmap_len, munmap_offset, munmap_len) < 0) {
        THROW_ERROR("first mmap and then munmap failed");
    }
    return 0;
}

int test_munmap_whose_range_intersects_with_multiple_mmap_regions() {
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;

    size_t mmap_len1 = 100 * PAGE_SIZE;
    void *mmap_addr1 = mmap(NULL, mmap_len1, prot, flags, -1, 0);
    if (mmap_addr1 == MAP_FAILED) {
        THROW_ERROR("mmap failed");
    }

    size_t mmap_len2 = 12 * PAGE_SIZE;
    void *mmap_addr2 = mmap(NULL, mmap_len2, prot, flags, -1, 0);
    if (mmap_addr2 == MAP_FAILED) {
        THROW_ERROR("mmap failed");
    }

    size_t mmap_min = MIN((size_t)mmap_addr1, (size_t)mmap_addr2);
    size_t mmap_max = MAX((size_t)mmap_addr1 + mmap_len1,
                          (size_t)mmap_addr2 + mmap_len2);

    void *munmap_addr = (void *)mmap_min;
    size_t munmap_len = mmap_max - mmap_min;
    if (munmap(munmap_addr, munmap_len) < 0) {
        THROW_ERROR("munmap failed");
    }
    if (check_buf_is_munmapped(munmap_addr, munmap_len) < 0) {
        THROW_ERROR("munmap does not really free the memory");
    }

    return 0;
}

int test_munmap_with_null_addr() {
    // Set the address for munmap to NULL!
    //
    // The man page of munmap states that "it is not an error if the indicated
    // range does not contain any mapped pages". This is not considered as
    // an error!
    void *munmap_addr = NULL;
    size_t munmap_len = PAGE_SIZE;
    if (munmap(munmap_addr, munmap_len) < 0) {
        THROW_ERROR("munmap failed");
    }
    return 0;
}

int test_munmap_with_zero_len() {
    void *munmap_addr = (void *)HINT_BEGIN;
    // Set the length for munmap to 0! This is invalid!
    size_t munmap_len = 0;
    if (munmap(munmap_addr, munmap_len) == 0) {
        THROW_ERROR("munmap with zero length should have failed");
    }
    return 0;
}

int test_munmap_with_non_page_aligned_len() {
    size_t mmap_len = 2 * PAGE_SIZE;
    size_t munmap_offset = 0;
    // Set the length for munmap to a non-page aligned value!
    //
    // The man page of munmap states that "the address addr must be a
    // multiple of the page size (but length need not be). All pages
    // containing a part of the indicated range are unmapped". So this is
    // not considered as an error!
    size_t munmap_len = 1 * PAGE_SIZE + 123;
    if (mmap_then_munmap(mmap_len, munmap_offset, munmap_len) < 0) {
        THROW_ERROR("first mmap and then munmap failed");
    }
    return 0;
}

// ============================================================================
// Test cases for mremap
// ============================================================================

int test_mremap() {
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;

    for (size_t len = PAGE_SIZE; len < MAX_MMAP_USED_MEMORY; len *= 2) {
        void *buf = mmap(NULL, len, prot, flags, -1, 0);
        if (buf == MAP_FAILED) {
            THROW_ERROR("mmap failed");
        }
        if (check_bytes_in_buf(buf, len, 0) < 0) {
            THROW_ERROR("the buffer is not initialized to zeros");
        }

        void *expand_buf = mremap(buf, len, 2 * len, MREMAP_MAYMOVE);
        if (expand_buf == MAP_FAILED) {
            THROW_ERROR("mremap with big size failed");
        }
        if (check_bytes_in_buf(expand_buf, len, 0) < 0) {
            THROW_ERROR("the old part of expand buffer is not zero");
        }
        memset(expand_buf, 'a', len * 2);

        void *shrink_buf = mremap(expand_buf, 2 * len, len, 0);
        if (shrink_buf == MAP_FAILED) {
            THROW_ERROR("mmap with small size failed");
        }
        if (check_bytes_in_buf(shrink_buf, len, 'a') < 0) {
            THROW_ERROR("the shrink buffer is not correct");
        }

        int ret = munmap(shrink_buf, len);
        if (ret < 0) {
            THROW_ERROR("munmap failed");
        }
    }
    return 0;
}

int test_mremap_subrange() {
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;

    size_t len = PAGE_SIZE * 4;
    void *buf = mmap(NULL, len, prot, flags, -1, 0);
    if (buf == MAP_FAILED) {
        THROW_ERROR("mmap failed");
    }
    if (check_bytes_in_buf(buf, len, 0) < 0) {
        THROW_ERROR("the buffer is not initialized to zeros");
    }
    /* remap a subrange in the buffer */
    void *new_part_buf = mremap(buf + len / 4, len / 4, len, MREMAP_MAYMOVE);
    if (new_part_buf == MAP_FAILED) {
        THROW_ERROR("mremap with subrange failed");
    }
    if (check_bytes_in_buf(new_part_buf, len / 4, 0) < 0) {
        THROW_ERROR("the old part of buffer is not zero");
    }
    void *rear_buf = buf + len / 2;
    /* now the length of rear buffer is (len / 2), remap the second part */
    void *new_part_rear_buf = mremap(rear_buf + len / 4, len / 4, len, MREMAP_MAYMOVE);
    if (new_part_rear_buf == MAP_FAILED) {
        THROW_ERROR("mremap with rear subrange failed");
    }
    if (check_bytes_in_buf(new_part_rear_buf, len / 4, 0) < 0) {
        THROW_ERROR("the old part of rear buffer is not zero");
    }
    int ret = munmap(buf, len / 4) || munmap(new_part_buf, len) ||
              munmap(rear_buf, len / 4) || munmap(new_part_rear_buf, len);
    if (ret < 0) {
        THROW_ERROR("munmap failed");
    }
    return 0;
}

// FIXME: may cause segfault on Linux
int test_mremap_with_fixed_addr() {
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;

    size_t len = PAGE_SIZE * 2;
    void *buf = mmap(NULL, len, prot, flags, -1, 0);
    if (buf == MAP_FAILED) {
        THROW_ERROR("mmap failed");
    }
    if (check_bytes_in_buf(buf, len, 0) < 0) {
        THROW_ERROR("the buffer is not initialized to zeros");
    }

    void *new_addr = buf + len * 2;
    void *new_buf = mremap(buf, len, len, MREMAP_FIXED, new_addr);
    if (new_buf != MAP_FAILED || errno != EINVAL) {
        THROW_ERROR("check mremap with invalid flags failed");
    }
    new_buf = mremap(buf, len, len, MREMAP_FIXED | MREMAP_MAYMOVE, buf);
    if (new_buf != MAP_FAILED || errno != EINVAL) {
        THROW_ERROR("check mremap with overlap addr failed");
    }
    new_buf = mremap(buf, len, len, MREMAP_FIXED | MREMAP_MAYMOVE, new_addr);
    if (new_buf == MAP_FAILED) {
        THROW_ERROR("mmap with a fixed address failed");
    }
    if (check_bytes_in_buf(new_buf, len, 0) < 0) {
        THROW_ERROR("the new buffer is not zero");
    }
    int ret = munmap(new_buf, len);
    if (ret < 0) {
        THROW_ERROR("munmap failed");
    }
    return 0;
}

// ============================================================================
// Test cases for mprotect
// ============================================================================

int test_mprotect_once() {
    // The memory permissions initially looks like below:
    //
    // Pages:            #0   #1   #2   #3
    // -------------------------------------
    // Memory perms:     [   ][   ][   ][   ]
    size_t total_len = 4; // in pages
    int init_prot = PROT_NONE;

    // The four settings for mprotect and its resulting memory perms.
    //
    // Pages:            #0   #1   #2   #3
    // -------------------------------------
    // Setting (i = 0):
    //  mprotect:        [RW ][RW ][RW ][RW ]
    //  result:          [RW ][RW ][RW ][RW ]
    // Setting (i = 1):
    //  mprotect:        [RW ]
    //  result:          [RW ][   ][   ][   ]
    // Setting (i = 2):
    //  mprotect:                  [RW ][RW ]
    //  result:          [   ][   ][RW ][RW ]
    // Setting (i = 3):
    //  mprotect:             [RW ][RW ]
    //  result:          [   ][RW ][RW ][   ]
    size_t lens[] = { 4, 1, 2, 2}; // in pages
    size_t offsets[] = { 0, 0, 2, 1}; // in pages
    for (int i = 0; i < ARRAY_SIZE(lens); i++) {
        int flags = MAP_PRIVATE | MAP_ANONYMOUS;
        void *buf = mmap(NULL, total_len * PAGE_SIZE, init_prot, flags, -1, 0);
        if (buf == MAP_FAILED) {
            THROW_ERROR("mmap failed");
        }

        size_t len = lens[i] * PAGE_SIZE;
        size_t offset = offsets[i] * PAGE_SIZE;
        int prot = PROT_READ | PROT_WRITE;
        void *tmp_buf = (char *)buf + offset;
        int ret = mprotect(tmp_buf, len, prot);
        if (ret < 0) {
            THROW_ERROR("mprotect failed");
        }

        ret = munmap(buf, total_len * PAGE_SIZE);
        if (ret < 0) {
            THROW_ERROR("munmap failed");
        }
    }

    return 0;
}

int test_mprotect_twice() {
    // The memory permissions initially looks like below:
    //
    // Pages:              #0   #1   #2   #3
    // -------------------------------------
    // Memory perms:       [   ][   ][   ][   ]
    size_t total_len = 4; // in pages
    int init_prot = PROT_NONE;

    // The four settings for mprotects and their results
    //
    // Pages:              #0   #1   #2   #3
    // -------------------------------------
    // Setting (i = 0):
    //  mprotect (j = 0):  [RW ][RW ]
    //  mprotect (j = 1):            [RW ][RW ]
    //  result:            [RW ][RW ][RW ][RW ]
    // Setting (i = 1):
    //  mprotect (j = 0):       [RW ]
    //  mprotect (j = 1):                 [RW ]
    //  result:            [   ][RW ][   ][RW ]
    // Setting (i = 2):
    //  mprotect (j = 0):       [RW ][RW ]
    //  mprotect (j = 1):       [ WX][ WX]
    //  result:            [   ][ WX][ WX][  ]
    // Setting (i = 3):
    //  mprotect (j = 0):       [RW ][RW ]
    //  mprotect (j = 1):       [   ]
    //  result:            [   ][   ][RW ][   ]
    size_t lens[][2] = {
        { 2, 2 },
        { 1, 1 },
        { 2, 2 },
        { 2, 1 }
    }; // in pages
    size_t offsets[][2] = {
        { 0, 2 },
        { 1, 3 },
        { 1, 1 },
        { 1, 1 }
    }; // in pages
    int prots[][2] = {
        { PROT_READ | PROT_WRITE, PROT_READ | PROT_WRITE },
        { PROT_READ | PROT_WRITE, PROT_READ | PROT_WRITE },
        { PROT_READ | PROT_WRITE, PROT_WRITE | PROT_EXEC },
        { PROT_READ | PROT_WRITE, PROT_NONE }
    };
    for (int i = 0; i < ARRAY_SIZE(lens); i++) {
        int flags = MAP_PRIVATE | MAP_ANONYMOUS;
        void *buf = mmap(NULL, total_len * PAGE_SIZE, init_prot, flags, -1, 0);
        if (buf == MAP_FAILED) {
            THROW_ERROR("mmap failed");
        }

        for (int j = 0; j < 2; j++) {
            size_t len = lens[i][j] * PAGE_SIZE;
            size_t offset = offsets[i][j] * PAGE_SIZE;
            int prot = prots[i][j];
            void *tmp_buf = (char *)buf + offset;
            int ret = mprotect(tmp_buf, len, prot);
            if (ret < 0) {
                THROW_ERROR("mprotect failed");
            }
        }

        int ret = munmap(buf, total_len * PAGE_SIZE);
        if (ret < 0) {
            THROW_ERROR("munmap failed");
        }
    }
    return 0;
}

int test_mprotect_triple() {
    // The memory permissions initially looks like below:
    //
    // Pages:              #0   #1   #2   #3
    // -------------------------------------
    // Memory perms:       [RWX][RWX][RWX][RWX]
    size_t total_len = 4; // in pages
    int init_prot = PROT_READ | PROT_WRITE | PROT_EXEC;

    // The four settings for mprotects and their results
    //
    // Pages:              #0   #1   #2   #3
    // -------------------------------------
    // Setting (i = 0):
    //  mprotect (j = 0):  [   ][   ]
    //  mprotect (j = 1):                 [   ]
    //  mprotect (j = 2):            [   ]
    //  result:            [   ][   ][   ][   ]
    size_t lens[][3] = {
        { 2, 1, 1 },
    }; // in pages
    size_t offsets[][3] = {
        { 0, 3, 2 },
    }; // in pages
    int prots[][3] = {
        { PROT_NONE, PROT_NONE, PROT_NONE },
    };
    for (int i = 0; i < ARRAY_SIZE(lens); i++) {
        int flags = MAP_PRIVATE | MAP_ANONYMOUS;
        void *buf = mmap(NULL, total_len * PAGE_SIZE, init_prot, flags, -1, 0);
        if (buf == MAP_FAILED) {
            THROW_ERROR("mmap failed");
        }

        for (int j = 0; j < 3; j++) {
            size_t len = lens[i][j] * PAGE_SIZE;
            size_t offset = offsets[i][j] * PAGE_SIZE;
            int prot = prots[i][j];
            void *tmp_buf = (char *)buf + offset;
            int ret = mprotect(tmp_buf, len, prot);
            if (ret < 0) {
                THROW_ERROR("mprotect failed");
            }
        }

        int ret = munmap(buf, total_len * PAGE_SIZE);
        if (ret < 0) {
            THROW_ERROR("munmap failed");
        }
    }
    return 0;
}

int test_mprotect_with_zero_len() {
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    void *buf = mmap(NULL, PAGE_SIZE, PROT_NONE, flags, -1, 0);
    if (buf == MAP_FAILED) {
        THROW_ERROR("mmap failed");
    }

    int ret = mprotect(buf, 0, PROT_NONE);
    if (ret < 0) {
        THROW_ERROR("mprotect failed");
    }

    ret = munmap(buf, PAGE_SIZE);
    if (ret < 0) {
        THROW_ERROR("munmap failed");
    }

    return 0;
}

int test_mprotect_with_invalid_addr() {
    int ret = mprotect(NULL, PAGE_SIZE, PROT_NONE);
    if (ret == 0 || errno != ENOMEM) {
        THROW_ERROR("using invalid addr should have failed");
    }
    return 0;
}

int test_mprotect_with_invalid_prot() {
    int invalid_prot = 0x1234; // invalid protection bits
    void *valid_addr = &invalid_prot;
    size_t valid_len = PAGE_SIZE;
    int ret = mprotect(valid_addr, valid_len, invalid_prot);
    if (ret == 0 || errno != EINVAL) {
        THROW_ERROR("using invalid addr should have failed");
    }
    return 0;
}

int test_mprotect_with_non_page_aligned_size() {
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    void *buf = mmap(NULL, PAGE_SIZE * 2, PROT_NONE, flags, -1, 0);
    if (buf == MAP_FAILED) {
        THROW_ERROR("mmap failed");
    }

    // Use raw syscall interface becase libc wrapper will handle non-page-aligned address
    // and will not cause failure.
    // Raw mprotect syscall with non-page-aligned address should fail.
    int ret = syscall(SYS_mprotect, buf + 10, PAGE_SIZE, PROT_WRITE);
    if (ret == 0 || errno != EINVAL) {
        THROW_ERROR("mprotect with non-page-aligned address should fail with EINVAL");
    }

    // According to man page of mprotect, this syscall require a page aligned start address, but the size could be any value.
    // Raw mprotect syscall with non-page-aligned size should succeed.
    ret = syscall(SYS_mprotect, buf, PAGE_SIZE + 100, PROT_WRITE);
    if (ret < 0) {
        THROW_ERROR("mprotect with non-page-aligned size failed");
    }

    // Mprotect succeeded and the pages are writable.
    *(char *)buf = 1;
    *(char *)(buf  + PAGE_SIZE) = 1;

    ret = munmap(buf, PAGE_SIZE * 2);
    if (ret < 0) {
        THROW_ERROR("munmap failed");
    }
    return 0;
}

int test_mprotect_multiple_vmas() {
    // Create multiple VMA with PROT_NONE
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    void *buf_a = mmap((void *)HINT_BEGIN, PAGE_SIZE * 2, PROT_NONE, flags, -1, 0);
    if (buf_a == MAP_FAILED || buf_a != (void *)HINT_BEGIN) {
        THROW_ERROR("mmap failed");
    }
    void *buf_b = mmap((void *)(HINT_BEGIN + 2 * PAGE_SIZE), PAGE_SIZE, PROT_NONE, flags, -1,
                       0);
    if (buf_b == MAP_FAILED || buf_b != (void *)(HINT_BEGIN + 2 * PAGE_SIZE)) {
        THROW_ERROR("mmap failed");
    }
    void *buf_c = mmap((void *)(HINT_BEGIN + 3 * PAGE_SIZE), PAGE_SIZE * 2, PROT_NONE, flags,
                       -1, 0);
    if (buf_c == MAP_FAILED || buf_c != (void *)(HINT_BEGIN + 3 * PAGE_SIZE)) {
        THROW_ERROR("mmap failed");
    }

    // Set a part of the ranges to read-write
    int ret = mprotect(buf_a + PAGE_SIZE, 3 * PAGE_SIZE, PROT_READ | PROT_WRITE);
    if (ret < 0) {
        THROW_ERROR("mprotect multiple vmas failed");
    }

    // Check if these ranges are writable
    *(char *)(buf_a + PAGE_SIZE) = 1;
    *(char *)(buf_b) = 1;
    *(char *)(buf_c) = 1;

    ret = munmap(buf_a, PAGE_SIZE * 5);
    if (ret < 0) {
        THROW_ERROR("munmap multiple vmas failed");
    }

    return 0;
}

int test_mprotect_grow_down() {
    int flags = MAP_PRIVATE | MAP_ANONYMOUS | MAP_GROWSDOWN;
    void *buf = mmap(0, PAGE_SIZE * 2, PROT_NONE, flags, -1, 0);
    if (buf == MAP_FAILED) {
        THROW_ERROR("mmap failed");
    }

    // Mprotect can use PROT_GROWSDOWN on a stack segment or a segment mapped with the MAP_GROWSDOWN flag set
    int ret = mprotect(buf, 2 * PAGE_SIZE,
                       PROT_READ | PROT_WRITE | PROT_EXEC | PROT_GROWSDOWN);
    if (ret < 0) {
        THROW_ERROR("mprotect  failed");
    }

    return 0;
}

int check_file_first_four_page(char *file_path, int first_page_val, int secend_page_val,
                               int third_page_val, int fourth_page_val) {
    int fd = open(file_path, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("file open failed");
    }
    if (check_file_with_repeated_bytes(fd, PAGE_SIZE, first_page_val) < 0) {
        THROW_ERROR("unexpected file content");
    }
    if (check_file_with_repeated_bytes(fd, PAGE_SIZE, secend_page_val) < 0) {
        THROW_ERROR("unexpected file content");
    }

    if (check_file_with_repeated_bytes(fd, PAGE_SIZE, third_page_val) < 0) {
        THROW_ERROR("unexpected file content\n");
    }

    if (check_file_with_repeated_bytes(fd, PAGE_SIZE, fourth_page_val) < 0) {
        THROW_ERROR("unexpectbed file content");
    }
    close(fd);
    return 0;
}

typedef int (* test_file_backed_mremap_fn_t) (void *, size_t, void **);

static int byte_val_0 = 0xff;
static int byte_val_1 = 0xab;
static int byte_val_2 = 0xcd;
static int byte_val_3 = 0xef;

int file_backed_mremap_simple(void *buf, size_t len, void **new_buf) {
    void *expand_buf = mremap(buf, len, 2 * len, 0);
    if (expand_buf == MAP_FAILED) {
        THROW_ERROR("mremap with big size failed");
    }
    // Check the value assigned before
    if (check_bytes_in_buf(expand_buf, len, byte_val_1) != 0 ) {
        THROW_ERROR("check expand_buf error");
    };
    // Check the value of second page which should be mapped from file
    if (check_bytes_in_buf(expand_buf + len, len, byte_val_0) != 0 ) {
        THROW_ERROR("check expand_buf error");
    };
    // Assign new value to the second page
    for (int i = len; i < len * 2; i++) { ((char *)expand_buf)[i] = byte_val_2; }

    expand_buf = mremap(expand_buf, len * 2, 4 * len, 0);
    if (expand_buf == MAP_FAILED) {
        THROW_ERROR("mremap with bigger size failed");
    }
    // Third and fourth page are not assigned any new value, so should still be 0.
    if (check_bytes_in_buf((void *)(expand_buf + len * 2), len * 2, 0) != 0) {
        THROW_ERROR("check buf content error");
    };

    // Assign new value  to the fourth page
    for (int i = len * 3; i < len * 4; i++) { ((char *)expand_buf)[i] = byte_val_3; }
    *new_buf = expand_buf;
    return 0;
}

int file_backed_mremap_mem_may_move(void *buf, size_t len, void **new_buf) {
    int prot = PROT_READ | PROT_WRITE;
    // Allocate a gap buffer to make sure mremap buf must move to a new range
    unsigned long gap_buf = (unsigned long) buf + len;
    assert(gap_buf % PAGE_SIZE == 0);
    void *ret = mmap((void *)gap_buf, PAGE_SIZE, prot,
                     MAP_ANONYMOUS | MAP_PRIVATE | MAP_FIXED, 0, 0);
    if ((unsigned long)ret != gap_buf) {
        THROW_ERROR("mmap gap_buf with prefered address failed");
    }

    void *expand_buf = mremap(buf, len, 2 * len, MREMAP_MAYMOVE);
    if (expand_buf == MAP_FAILED) {
        THROW_ERROR("mremap with big size failed");
    }
    // Check the value assigned before
    if (check_bytes_in_buf(expand_buf, len, byte_val_1) != 0 ) {
        THROW_ERROR("check expand_buf error");
    };
    // Check the value of second page which should be mapped from file
    if (check_bytes_in_buf(expand_buf + len, len, byte_val_0) != 0 ) {
        THROW_ERROR("check expand_buf error");
    };
    // Assign new value to the second page
    for (int i = len; i < len * 2; i++) { ((char *)expand_buf)[i] = byte_val_2; }

    // Mremap to a new fixed address
    unsigned long fixed_addr = (unsigned long) expand_buf + 2 * len;
    ret = mremap(expand_buf, len * 2, 4 * len, MREMAP_FIXED | MREMAP_MAYMOVE,
                 (void *)fixed_addr);
    if ((unsigned long)ret != fixed_addr) {
        THROW_ERROR("mremap with fixed address and more big size failed");
    }
    // Third and fourth page are not assigned any new value, so should still be 0.
    if (check_bytes_in_buf((void *)(fixed_addr + len * 2), len * 2, 0) != 0) {
        THROW_ERROR("check buf content error");
    };

    // Assign new value  to the fourth page
    for (int i = len * 3; i < len * 4; i++) { ((char *)fixed_addr)[i] = byte_val_3; }

    int rc = munmap((void *)gap_buf, PAGE_SIZE);
    if (rc < 0) {
        THROW_ERROR("munmap gap_buf failed");
    }

    *new_buf = (void *)fixed_addr;
    return 0;
}

int _test_file_backed_mremap(test_file_backed_mremap_fn_t fn) {
    int prot = PROT_READ | PROT_WRITE;
    size_t len = PAGE_SIZE;
    char *file_path = "/tmp/test";

    // O_TRUNC is not supported by Occlum yet.
    remove(file_path);
    int fd = open(file_path, O_RDWR | O_CREAT | O_NOFOLLOW | O_CLOEXEC | O_TRUNC, 0600);
    if (fd < 0) {
        THROW_ERROR("open file error");
    }
    fallocate(fd, 0, 0, len * 4);
    fill_file_with_repeated_bytes(fd, len * 2, byte_val_0);

    void *buf = mmap(0, len, prot, MAP_SHARED, fd, 0);
    if (buf == MAP_FAILED) {
        THROW_ERROR("mmap failed");
    }
    for (int i = 0; i < len; i++) { ((char *)buf)[i] = byte_val_1; }

    void *expand_buf = 0;
    int ret = fn(buf, len, &expand_buf);
    if (ret != 0) {
        THROW_ERROR("mremap test failed");
    }

    int rc = msync((void *)expand_buf, 4 * len, MS_SYNC);
    if (rc < 0) {
        THROW_ERROR("msync failed");
    }
    rc = munmap((void *)expand_buf, 4 * len);
    if (rc < 0) {
        THROW_ERROR("munmap failed");
    }

    close(fd);

    return check_file_first_four_page(file_path, byte_val_1, byte_val_2, 0, byte_val_3);;
}

int test_file_backed_mremap() {
    return _test_file_backed_mremap(file_backed_mremap_simple);
}

int test_file_backed_mremap_mem_may_move() {
    return _test_file_backed_mremap(file_backed_mremap_mem_may_move);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_anonymous_mmap),
    TEST_CASE(test_anonymous_mmap_randomly),
    TEST_CASE(test_anonymous_mmap_randomly_with_good_hints),
    TEST_CASE(test_anonymous_mmap_with_bad_hints),
    TEST_CASE(test_anonymous_mmap_with_zero_len),
    TEST_CASE(test_anonymous_mmap_with_non_page_aligned_len),
    TEST_CASE(test_private_file_mmap),
    TEST_CASE(test_private_file_mmap_with_offset),
    TEST_CASE(test_private_file_mmap_with_invalid_fd),
    TEST_CASE(test_private_file_mmap_with_non_page_aligned_offset),
    TEST_CASE(test_shared_file_mmap_flushing_with_msync),
    TEST_CASE(test_shared_file_mmap_flushing_with_munmap),
    TEST_CASE(test_shared_file_mmap_flushing_with_fdatasync),
    TEST_CASE(test_shared_file_mmap_flushing_with_fsync),
    TEST_CASE(test_fixed_mmap_that_does_not_override_any_mmaping),
    TEST_CASE(test_fixed_mmap_that_overrides_existing_mmaping),
    TEST_CASE(test_fixed_mmap_with_non_page_aligned_addr),
    TEST_CASE(test_munmap_whose_range_is_a_subset_of_a_mmap_region),
    TEST_CASE(test_munmap_whose_range_is_a_superset_of_a_mmap_region),
    TEST_CASE(test_munmap_whose_range_intersects_with_a_mmap_region),
    TEST_CASE(test_munmap_whose_range_intersects_with_no_mmap_regions),
    TEST_CASE(test_munmap_whose_range_intersects_with_multiple_mmap_regions),
    TEST_CASE(test_munmap_with_null_addr),
    TEST_CASE(test_munmap_with_zero_len),
    TEST_CASE(test_munmap_with_non_page_aligned_len),
    TEST_CASE(test_mremap),
    TEST_CASE(test_mremap_subrange),
    TEST_CASE(test_mremap_with_fixed_addr),
    TEST_CASE(test_file_backed_mremap),
    TEST_CASE(test_file_backed_mremap_mem_may_move),
    TEST_CASE(test_mprotect_once),
    TEST_CASE(test_mprotect_twice),
    TEST_CASE(test_mprotect_triple),
    TEST_CASE(test_mprotect_with_zero_len),
    TEST_CASE(test_mprotect_with_invalid_addr),
    TEST_CASE(test_mprotect_with_invalid_prot),
    TEST_CASE(test_mprotect_with_non_page_aligned_size),
    TEST_CASE(test_mprotect_multiple_vmas),
    TEST_CASE(test_mprotect_grow_down),
};

int main() {
    if (test_suite_init() < 0) {
        THROW_ERROR("test_suite_init failed");
    }

    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
