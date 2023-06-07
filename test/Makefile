CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
PROJECT_DIR := $(realpath $(CUR_DIR)/../)
SGX_MODE ?= HW

# Repeat times for make test. Default to 1
STRESS_TEST_TIMES ?= 1

C_SRCS := $(wildcard */*.c */*.h)
CXX_SRCS := $(wildcard */*.cc)

C_FORMATTER := $(PROJECT_DIR)/tools/c_formatter

BUILD_DIR := $(PROJECT_DIR)/build

PASS_LOG = $(BUILD_DIR)/test/.pass
FAIL_LOG = $(BUILD_DIR)/test/.fail

# Dependencies: need to be compiled but not to run by any Makefile target
TEST_DEPS := client data_sink naughty_child wait_child

# Tests: need to be compiled and run by test-% target
TESTS ?= env empty hello_world malloc fs_perms getpid spawn sched pipe time \
    readdir mkdir open stat link symlink tls pthread system_info rlimit \
    server server_epoll unix_socket cout hostfs cpuid rdtsc device sleep exit_group posix_flock \
    ioctl fcntl eventfd emulate_syscall signal prctl rename procfs wait flock async_sfs \
    spawn_attribute exec statfs random pgrp vfork mount sysinfo timerfd shm epoll \
    netlink brk disk

# Legacy tests: Some cases still applied on legacy configuration due to functionality cause
TESTS_LEGACY ?= mmap file truncate chmod chown access umask utimes

# Benchmarks: need to be compiled and run by bench-% target
BENCHES := spawn_and_exit_latency pipe_throughput unix_socket_throughput

# Occlum bin path
OCCLUM_BIN_PATH ?= $(BUILD_DIR)/bin

# Top-level Makefile targets
BUILD_TARGETS := $(TEST_DEPS) $(TESTS) $(TESTS_LEGACY) $(BENCHES)
TEST_TARGETS := $(TESTS:%=test-%)
TEST_LEGACY_TARGETS := $(TESTS_LEGACY:%=test-%)
BENCH_TARGETS := $(BENCHES:%=bench-%)
.PHONY: all prebuild build postbuild test clean $(BUILD_TARGETS) $(TEST_TARGETS) $(BENCH_TARGETS) format format-check

# Use echo program instead of built-in echo command in shell. This ensures
# that echo can recognize escaped sequences (with -e argument) regardless of
# the specific shell (e.g., bash, zash, etc.)
ECHO := /bin/echo -e
# Shell escaped sequences for colorful output
CYAN := \033[1;36m
GREEN := \033[1;32m
RED := \033[1;31m
NO_COLOR := \033[0m

ifneq ($(TEST_CPUS), )
	cpu_options = --cpus $(TEST_CPUS)
endif

#############################################################################
# Build targets
#############################################################################

all: build

build: prebuild $(BUILD_TARGETS) postbuild

prebuild:
	@$(RM) -rf $(BUILD_DIR)/test
	@mkdir -p $(BUILD_DIR)/test
	@cd $(BUILD_DIR)/test && \
		$(OCCLUM_BIN_PATH)/occlum init
	@cp Occlum.yaml $(BUILD_DIR)/test/

prebuild-legacy:
	@$(RM) -rf $(BUILD_DIR)/test
	@mkdir -p $(BUILD_DIR)/test
	@cd $(BUILD_DIR)/test && \
		$(OCCLUM_BIN_PATH)/occlum init
	@cp -f Occlum-legacy.yaml $(BUILD_DIR)/test/Occlum.yaml

$(BUILD_TARGETS): %:
	@$(ECHO) "$(CYAN)BUILD TEST => $@$(NO_COLOR)"
	@$(MAKE) --no-print-directory -C $@
	@$(ECHO) "$(GREEN)DONE$(NO_COLOR)"

postbuild:
	@cd $(BUILD_DIR)/test && \
		$(OCCLUM_BIN_PATH)/occlum build -f

#############################################################################
# Test targets
#############################################################################

test:
	@$(MAKE) test-common

test-glibc:
	@OCCLUM_TEST_GLIBC=1 $(MAKE) test-common

test-common:
	@if [ -n "$(times)" ] && [ "$(times)" -eq "$(times)" ] 2>/dev/null; then \
		export STRESS_TEST_TIMES=$(times); \
	fi ; \
	$(MAKE) test-internal

test-internal: build pretest $(TEST_TARGETS) posttest

test-legacy:
	@OCCLUM_TEST_GLIBC=1 $(MAKE) test-legacy-internal

test-legacy-internal: prebuild-legacy $(BUILD_TARGETS) postbuild pretest $(TEST_LEGACY_TARGETS) posttest

pretest:
	@$(RM) $(PASS_LOG) $(FAIL_LOG)
	@cd $(BUILD_DIR)/test && \
		$(OCCLUM_BIN_PATH)/occlum start $(cpu_options)

# Restart server if test failed
$(TEST_TARGETS): test-%: %
	@touch $(PASS_LOG) $(FAIL_LOG)
	@for i in $$(seq 1 $(STRESS_TEST_TIMES)); \
	do \
		$(ECHO) "$(CYAN)RUN TEST => $<$(NO_COLOR)"; \
		$(MAKE) --no-print-directory -C $< test ; \
		if [ $$? -eq 0 ] ; then \
			$(ECHO) "$(GREEN)PASS $(NO_COLOR)" ; \
			$(ECHO) "$< PASS $$i" >> $(PASS_LOG) ; \
		else \
			$(ECHO) "$(RED)FAILED$(NO_COLOR)" ; \
			$(ECHO) "$< FAILED $$i" >> $(FAIL_LOG) ; \
			cd $(BUILD_DIR)/test && \
			$(OCCLUM_BIN_PATH)/occlum start ; \
			cd -; \
		fi ; \
	done

$(TEST_LEGACY_TARGETS): test-%: %
	@touch $(PASS_LOG) $(FAIL_LOG)
	@$(ECHO) "$(CYAN)RUN TEST => $<$(NO_COLOR)"
	@$(MAKE) --no-print-directory -C $< test ; \
		if [ $$? -eq 0 ] ; then \
			$(ECHO) "$(GREEN)PASS $(NO_COLOR)" ; \
			$(ECHO) "$< PASS " >> $(PASS_LOG) ; \
		else \
			$(ECHO) "$(RED)FAILED$(NO_COLOR)" ; \
			$(ECHO) "$< FAILED " >> $(FAIL_LOG) ; \
			cd $(BUILD_DIR)/test && \
			$(OCCLUM_BIN_PATH)/occlum start ; \
			cd -; \
		fi ; \

posttest:
	@cd $(BUILD_DIR)/test && \
		$(OCCLUM_BIN_PATH)/occlum stop
	@PASS_NUM=$(shell wc -l < $(PASS_LOG)); \
		FAIL_NUM=$(shell wc -l < $(FAIL_LOG)); \
		$(ECHO) "\nTotal:" ; \
		$(ECHO) "$(GREEN)PASS: $$PASS_NUM  $(RED)FAILED: $$FAIL_NUM $(NO_COLOR)" ; \
		if [ $$FAIL_NUM -ne 0 ] ; then \
			$(ECHO) "\nFAILED TESTS:$(RED)" ; \
			cat $(FAIL_LOG); \
			$(ECHO) "$(NO_COLOR)====================="; \
			exit 1; \
		fi ;

#############################################################################
# Benchmark targets
#############################################################################

bench: build $(BENCH_TARGETS)

$(BENCH_TARGETS): bench-%: %
	@$(ECHO) "$(CYAN)RUN BENCH => $<$(NO_COLOR)"
	@$(MAKE) --no-print-directory -C $< test ; \
		if [ $$? -eq 0 ] ; then \
			$(ECHO) "$(GREEN)DONE$(NO_COLOR)" ; \
		else \
			$(ECHO) "$(RED)FAILED$(NO_COLOR)" ; \
		fi ;

#############################################################################
# Misc
#############################################################################

format: $(C_SRCS) $(CXX_SRCS)
	@$(C_FORMATTER) $^

format-check: $(C_SRCS) $(CXX_SRCS)
	@$(C_FORMATTER) --check $^

clean:
	@$(RM) -rf $(BUILD_DIR)/test
