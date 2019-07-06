CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
PROJECT_DIR := $(realpath $(CUR_DIR)/../)

# Dependencies: need to be compiled but not to run by any Makefile target
TEST_DEPS := dev_null
# Tests: need to be compiled and run by test-% target
TESTS := empty env hello_world malloc mmap file getpid spawn pipe time \
	truncate readdir mkdir link tls pthread uname rlimit client server \
	server_epoll unix_socket cout hostfs cpuid rdtsc device
# Benchmarks: need to be compiled and run by bench-% target
BENCHES := spawn_and_exit_latency pipe_throughput unix_socket_throughput

# Top-level Makefile targets
BUILD_TARGETS := $(TEST_DEPS) $(TESTS) $(BENCHES)
TEST_TARGETS := $(TESTS:%=test-%)
BENCH_TARGETS := $(BENCHES:%=bench-%)
CLEAN_TARGETS := $(BUILD_TARGETS:%=clean-%)
.PHONY: all build test clean sefs $(BUILD_TARGETS) $(TEST_TARGETS) $(BENCH_TARGETS) $(CLEAN_TARGETS)

# Use echo program instead of built-in echo command in shell. This ensures
# that echo can recognize escaped sequences (with -e argument) regardless of
# the specific shell (e.g., bash, zash, etc.)
ECHO := /bin/echo -e
# Shell escaped sequences for colorful output
CYAN := \033[1;36m
GREEN := \033[1;32m
RED := \033[1;31m
NO_COLOR := \033[0m

FS_PATH := fs
SEFS_PATH := sefs

#############################################################################
# Build targets
#############################################################################

all: build

build: $(BUILD_TARGETS) sefs

$(BUILD_TARGETS): %:
	@$(ECHO) "$(CYAN)BUILD TEST => $@$(NO_COLOR)"
	@$(MAKE) --no-print-directory -C $@
	@$(ECHO) "$(GREEN)DONE$(NO_COLOR)"

sefs:
	@$(RM) -rf $(SEFS_PATH)
	@cp /lib/ld-musl-x86_64.so.1 $(FS_PATH)/ld.so
	@cd $(PROJECT_DIR)/deps/sefs/sefs-fuse/bin/ && \
		./app \
			$(CUR_DIR)/$(SEFS_PATH) \
			$(CUR_DIR)/$(FS_PATH) \
			zip
	@echo "SEFS => $@"

#############################################################################
# Test targets
#############################################################################

test: build $(TEST_TARGETS)

pal: $(PROJECT_DIR)/src/pal/pal
	@cp $< pal

libocclum.signed.so: $(PROJECT_DIR)/src/libos/libocclum.signed.so
	@cp $< libocclum.signed.so

$(TEST_TARGETS): test-%: % pal libocclum.signed.so
	@$(ECHO) "$(CYAN)RUN TEST => $<$(NO_COLOR)"
	@$(MAKE) --no-print-directory -C $< test ; \
		if [ $$? -eq 0 ] ; then \
			$(ECHO) "$(GREEN)PASS$(NO_COLOR)" ; \
		else \
			$(ECHO) "$(RED)FAILED$(NO_COLOR)" ; \
		fi ;

#############################################################################
# Benchmark targets
#############################################################################

bench: build $(BENCH_TARGETS)

$(BENCH_TARGETS): bench-%: % pal libocclum.signed.so
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

clean: $(CLEAN_TARGETS)
	@$(RM) -f pal libocclum.signed.so
	@$(RM) -rf $(FS_PATH) $(SEFS_PATH)

$(CLEAN_TARGETS): clean-%:
	@$(MAKE) --no-print-directory -C $(patsubst clean-%,%,$@) clean
