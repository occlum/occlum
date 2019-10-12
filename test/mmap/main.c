#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>
#include <assert.h>
#include <string.h>
#include <fcntl.h>
#include "test.h"

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

static int fill_file_with_repeated_bytes(int fd, size_t len, int byte_val) {
    char buf[PAGE_SIZE];
    memset(buf, byte_val, sizeof(buf));

    size_t remain_bytes = len;
    while (remain_bytes > 0) {
        int to_write_bytes = MIN(sizeof(buf), remain_bytes);
        int written_bytes = write(fd, buf, to_write_bytes);
        if (written_bytes != to_write_bytes) {
            throw_error("file write failed");
        }
        remain_bytes -= written_bytes;
    }

    return 0;
}

static int check_bytes_in_buf(char* buf, size_t len, int expected_byte_val) {
    for (size_t bi = 0; bi < len; bi++) {
        if (buf[bi] != (char)expected_byte_val) {
            printf("check_bytes_in_buf: expect %02X, but found %02X, at offset %lu\n",
                    (unsigned char)expected_byte_val, (unsigned char)buf[bi], bi);
            return -1;
        }
    }
    return 0;
}

static void* get_a_stack_ptr() {
    volatile int a = 0;
    return (void*) &a;
}

// ============================================================================
// Test suite initialization
// ============================================================================

// Get a valid range of address hints for mmap
static int get_a_valid_range_of_hints(size_t *hint_begin, size_t *hint_end) {
    size_t big_buf_len = MAX_MMAP_USED_MEMORY;
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    void* big_buf = mmap(NULL, big_buf_len, prot, flags, -1, 0);
    if (big_buf == MAP_FAILED) {
        throw_error("mmap failed");
    }
    int ret = munmap(big_buf, big_buf_len);
    if (ret < 0) {
        throw_error("munmap failed");
    }
    *hint_begin = (size_t)big_buf;
    *hint_end = *hint_begin + big_buf_len;
    return 0;
}

static size_t HINT_BEGIN, HINT_END;

int test_suite_init() {
    if (get_a_valid_range_of_hints(&HINT_BEGIN, &HINT_END) < 0) {
        throw_error("get_a_valid_range_of_hints failed");
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
        void* buf = mmap(NULL, len, prot, flags, -1, 0);
        if (buf == MAP_FAILED) {
            throw_error("mmap failed");
        }

        if (check_bytes_in_buf(buf, len, 0) < 0) {
            throw_error("the buffer is not initialized to zeros");
        }

        int ret = munmap(buf, len);
        if (ret < 0) {
            throw_error("munmap failed");
        }
    }
    return 0;
}

int test_anonymous_mmap_randomly() {
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;

    void* bufs[16] = {NULL};
    size_t lens[16];
    size_t num_bufs = 0;
    size_t used_memory = 0;

    for (int i = 0; i < 5; i++) {
        // Phrase 1: do mmap with random sizes until no more buffers or memory
        for (num_bufs = 0;
             num_bufs < ARRAY_SIZE(bufs) && used_memory < MAX_MMAP_USED_MEMORY;
             num_bufs++)
        {
            // Choose the mmap size randomly
            size_t len = rand() % (MAX_MMAP_USED_MEMORY - used_memory) + 1;
            len = ALIGN_UP(len, PAGE_SIZE);

            // Do mmap
            void* buf = mmap(NULL, len, prot, flags, -1, 0);
            if (buf == MAP_FAILED) {
                throw_error("mmap failed");
            }
            bufs[num_bufs] = buf;
            lens[num_bufs] = len;

            // Update memory usage
            used_memory += len;
        }

        // Phrase 2: do munmap to free all memory mapped memory
        for (int bi = 0; bi < num_bufs; bi++) {
            void* buf = bufs[bi];
            size_t len = lens[bi];
            int ret = munmap(buf, len);
            if (ret < 0) {
                throw_error("munmap failed");
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
        len = ALIGN_UP(len+1, PAGE_SIZE);

        void* addr = mmap((void*)hint, len, prot, flags, -1, 0);
        if (addr != (void*)hint) {
            throw_error("mmap with hint failed");
        }

        int ret = munmap(addr, len);
        if (ret < 0) {
            throw_error("munmap failed");
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
        void* bad_hint = (void*)bad_hints[hi];
        void* addr = mmap(bad_hint, len, prot, flags, -1, 0);
        if (addr == MAP_FAILED) {
            throw_error("mmap should have tolerated a bad hint");
        }
        if (addr == bad_hint) {
            throw_error("mmap should not have accepted a bad hint");
        }
        int ret = munmap(addr, len);
        if (ret < 0) {
            throw_error("munmap failed");
        }
    }
    return 0;
}

int test_anonymous_mmap_with_zero_len() {
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    int len = 0; // invalid!
    void* buf = mmap(NULL, len, prot, flags, -1, 0);
    if (buf != MAP_FAILED) {
        throw_error("mmap with zero len should have been failed");
    }
    return 0;
}

int test_anonymous_mmap_with_non_page_aligned_len() {
    int len = PAGE_SIZE + 17; // length need not to be page aligned!
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    void* buf = mmap(NULL, len, prot, flags, -1, 0);
    if (buf == MAP_FAILED) {
        throw_error("mmap with non-page aligned len should have worked");
    }

    // Even the length is not page aligned, the page mmaping is done in pages
    if (check_bytes_in_buf(buf, ALIGN_UP(len, PAGE_SIZE), 0) < 0) {
        throw_error("the buffer is not initialized to zeros");
    }

    int ret = munmap(buf, len);
    if (ret < 0) {
        throw_error("munmap failed");
    }
    return 0;
}

// ============================================================================
// Test cases for file-backed mmap
// ============================================================================

int test_file_mmap() {
    const char* file_path = "/root/mmap_file.data";
    int fd = open(file_path, O_CREAT | O_TRUNC | O_WRONLY, 0644);
    if (fd < 0) {
        throw_error("file creation failed");
    }
    int file_len = 12 * KB + 128;
    int byte_val = 0xab;
    fill_file_with_repeated_bytes(fd, file_len, byte_val);
    close(fd);

    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE;
    fd = open(file_path, O_RDONLY);
    if (fd < 0) {
        throw_error("file open failed");
    }
    off_t offset = 0;
    for (size_t len = PAGE_SIZE; len <= file_len; len *= 2) {
        char* buf = mmap(NULL, len, prot, flags, fd, offset);
        if (buf == MAP_FAILED) {
            throw_error("mmap failed");
        }

        if (check_bytes_in_buf(buf, len, byte_val) < 0) {
            throw_error("the buffer is not initialized according to the file");
        }

        int ret = munmap(buf, len);
        if (ret < 0) {
            throw_error("munmap failed");
        }
    }
    close(fd);
    unlink(file_path);
    return 0;
}

int test_file_mmap_with_offset() {
    const char* file_path = "/root/mmap_file.data";
    int fd = open(file_path, O_CREAT | O_TRUNC | O_RDWR, 0644);
    if (fd < 0) {
        throw_error("file creation failed");
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
    char* buf = mmap(NULL, len, prot, flags, fd, offset);
    if (buf == MAP_FAILED) {
        throw_error("mmap failed");
    }

    char* buf_cursor = buf;
    if (check_bytes_in_buf(buf_cursor, first_len - offset, first_val) < 0) {
        throw_error("the buffer is not initialized according to the file");
    }
    buf_cursor += first_len - offset;
    if (check_bytes_in_buf(buf_cursor, second_len, second_val) < 0) {
        throw_error("the buffer is not initialized according to the file");
    }
    buf_cursor += second_len;
    if (check_bytes_in_buf(buf_cursor, ALIGN_UP(len, PAGE_SIZE) - (buf_cursor - buf), 0) < 0) {
        throw_error("the remaining of the last page occupied by the buffer is not initialized to zeros");
    }

    int ret = munmap(buf, len);
    if (ret < 0) {
        throw_error("munmap failed");
    }

    close(fd);
    unlink(file_path);
    return 0;
}

int test_file_mmap_with_invalid_fd() {
    size_t len = PAGE_SIZE;
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE;
    int fd = 1234; // invalid!
    off_t offset = 0;
    void* buf = mmap(NULL, len, prot, flags, fd, offset);
    if (buf != MAP_FAILED) {
        throw_error("file mmap with an invalid fd should have been failed");
    }
    return 0;
}

int test_file_mmap_with_non_page_aligned_offset() {
    const char* file_path = "/root/mmap_file.data";
    int fd = open(file_path, O_CREAT | O_TRUNC | O_RDWR, 0644);
    if (fd < 0) {
        throw_error("file creation failed");
    }
    int file_len = 12 * KB + 128;
    int byte_val = 0xab;
    fill_file_with_repeated_bytes(fd, file_len, byte_val);

    size_t len = PAGE_SIZE;
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    off_t  offset = PAGE_SIZE + 127; // Invalid!
    void* buf = mmap(NULL, len, prot, flags, fd, offset);
    if (buf != MAP_FAILED) {
        throw_error("mmap with non-page aligned len should have been failed");
    }

    close(fd);
    unlink(file_path);
    return 0;
}

// TODO: what if offset > file size or offset + len > file size?

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
    void* addr = mmap((void*)hint, len, prot, flags, -1, 0);
    if (addr != (void*)hint) {
        throw_error("mmap with fixed address failed");
    }

    int ret = munmap(addr, len);
    if (ret < 0) {
        throw_error("munmap failed");
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
    void* parent_buf = mmap(NULL, parent_len, prot, flags, -1, 0);
    if (parent_buf == MAP_FAILED) {
        throw_error("mmap for parent failed");
    }
    int parent_val = 0xab;
    memset(parent_buf, parent_val, parent_len);

    // Allocate child_buf
    void* child_buf = (char*)parent_buf + pre_child_len;
    if (mmap(child_buf, child_len, prot, flags | MAP_FIXED, -1, 0) != child_buf) {
        throw_error("mmap with fixed address failed");
    }

    // Check that child_buf, which overrides parent_buf, is initialized to zeros
    if (check_bytes_in_buf(child_buf, child_len, 0) < 0) {
        throw_error("the content of child mmap memory is not initialized");
    }
    // Check that the rest of parent_buf are kept intact
    if (check_bytes_in_buf((char*)child_buf - pre_child_len,
                            pre_child_len, parent_val) < 0 ||
        check_bytes_in_buf((char*)child_buf + child_len,
                            post_child_len, parent_val) < 0) {
        throw_error("the content of parent mmap memory is broken");
    }

    // Deallocate parent_buf along with child_buf
    int ret = munmap(parent_buf, parent_len);
    if (ret < 0) {
        throw_error("munmap failed");
    }

    return 0;
}

int test_fixed_mmap_with_non_page_aligned_addr() {
    size_t hint = HINT_BEGIN + 123; // Not aligned!
    size_t len = 1 * PAGE_SIZE;
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED;
    void* addr = mmap((void*)hint, len, prot, flags, -1, 0);
    if (addr != MAP_FAILED) {
        throw_error("fixed mmap with non-page aligned hint should have failed");
    }
    return 0;
}

// ============================================================================
// Test cases for munmap
// ============================================================================

static int check_buf_is_munmapped(void* target_addr, size_t len) {
    // The trivial case of zero-len meory region is considered as unmapped
    if (len == 0) return 0;

    // If the target_addr is not already mmaped, it should succeed to use it as
    // a hint for mmap.
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;
    void* real_addr = mmap(target_addr, len, prot, flags, -1, 0);
    if (real_addr != target_addr) {
        throw_error("address is already mmaped");
    }
    munmap(target_addr, len);
    return 0;
}

static int mmap_then_munmap(size_t mmap_len, ssize_t munmap_offset, size_t munmap_len) {
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED;
    // Make sure that we are manipulating memory between [HINT_BEGIN, HINT_END)
    void* mmap_addr = (void*)(munmap_offset >= 0 ? HINT_BEGIN : HINT_BEGIN - munmap_offset);
    if (mmap(mmap_addr, mmap_len, prot, flags, -1, 0) != mmap_addr) {
        throw_error("mmap failed");
    }

    void* munmap_addr = (char*)mmap_addr + munmap_offset;
    if (munmap(munmap_addr, munmap_len) < 0) {
        throw_error("munmap failed");
    }
    if (check_buf_is_munmapped(munmap_addr, munmap_len) < 0) {
        throw_error("munmap does not really free the memory");
    }

    // Make sure that when this function returns, there are no memory mappings
    // within [HINT_BEGIN, HINT_END)
    if (munmap((void*)HINT_BEGIN, HINT_END - HINT_BEGIN) < 0) {
        throw_error("munmap failed");
    }
    return 0;
}

int test_munmap_whose_range_is_a_subset_of_a_mmap_region() {
    size_t mmap_len = 4 * PAGE_SIZE;
    ssize_t munmap_offset = 1 * PAGE_SIZE;
    size_t munmap_len = 2 * PAGE_SIZE;
    if (mmap_then_munmap(mmap_len, munmap_offset, munmap_len) < 0) {
        throw_error("first mmap and then munmap failed");
    }
    return 0;
}

int test_munmap_whose_range_is_a_superset_of_a_mmap_region() {
    size_t mmap_len = 4 * PAGE_SIZE;
    ssize_t munmap_offset = -2 * PAGE_SIZE;
    size_t munmap_len = 7 * PAGE_SIZE;
    if (mmap_then_munmap(mmap_len, munmap_offset, munmap_len) < 0) {
        throw_error("first mmap and then munmap failed");
    }
    return 0;
}

int test_munmap_whose_range_intersects_with_a_mmap_region() {
    size_t mmap_len = 200 * PAGE_SIZE;
    size_t munmap_offset = 100 * PAGE_SIZE + 10 * PAGE_SIZE;
    size_t munmap_len = 4 * PAGE_SIZE;
    if (mmap_then_munmap(mmap_len, munmap_offset, munmap_len) < 0) {
        throw_error("first mmap and then munmap failed");
    }
    return 0;
}

int test_munmap_whose_range_intersects_with_no_mmap_regions() {
    size_t mmap_len = 1 * PAGE_SIZE;
    size_t munmap_offset = 1 * PAGE_SIZE;
    size_t munmap_len = 1 * PAGE_SIZE;
    if (mmap_then_munmap(mmap_len, munmap_offset, munmap_len) < 0) {
        throw_error("first mmap and then munmap failed");
    }
    return 0;
}

int test_munmap_whose_range_intersects_with_multiple_mmap_regions() {
    int prot = PROT_READ | PROT_WRITE;
    int flags = MAP_PRIVATE | MAP_ANONYMOUS;

    size_t mmap_len1 = 100 * PAGE_SIZE;
    void* mmap_addr1 = mmap(NULL, mmap_len1, prot, flags, -1, 0);
    if (mmap_addr1 == MAP_FAILED) {
        throw_error("mmap failed");
    }

    size_t mmap_len2 = 12 * PAGE_SIZE;
    void* mmap_addr2 = mmap(NULL, mmap_len2, prot, flags, -1, 0);
    if (mmap_addr2 == MAP_FAILED) {
        throw_error("mmap failed");
    }

    size_t mmap_min = MIN((size_t)mmap_addr1, (size_t)mmap_addr2);
    size_t mmap_max = MAX((size_t)mmap_addr1 + mmap_len1,
                          (size_t)mmap_addr2 + mmap_len2);

    void* munmap_addr = (void*)mmap_min;
    size_t munmap_len = mmap_max - mmap_min;
    if (munmap(munmap_addr, munmap_len) < 0) {
        throw_error("munmap failed");
    }
    if (check_buf_is_munmapped(munmap_addr, munmap_len) < 0) {
        throw_error("munmap does not really free the memory");
    }

    return 0;
}

int test_munmap_with_null_addr() {
    // Set the address for munmap to NULL!
    //
    // The man page of munmap states that "it is not an error if the indicated
    // range does not contain any mapped pages". This is not considered as
    // an error!
    void* munmap_addr = NULL;
    size_t munmap_len = PAGE_SIZE;
    if (munmap(munmap_addr, munmap_len) < 0) {
        throw_error("munmap failed");
    }
    return 0;
}

int test_munmap_with_zero_len() {
    void* munmap_addr = (void*)HINT_BEGIN;
    // Set the length for munmap to 0! This is invalid!
    size_t munmap_len = 0;
    if (munmap(munmap_addr, munmap_len) == 0) {
        throw_error("munmap with zero length should have failed");
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
        throw_error("first mmap and then munmap failed");
    }
    return 0;
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
    TEST_CASE(test_file_mmap),
    TEST_CASE(test_file_mmap_with_offset),
    TEST_CASE(test_file_mmap_with_invalid_fd),
    TEST_CASE(test_file_mmap_with_non_page_aligned_offset),
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
    TEST_CASE(test_munmap_with_non_page_aligned_len)
};

int main() {
    if (test_suite_init() < 0) {
        throw_error("test_suite_init failed");
    }

    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
