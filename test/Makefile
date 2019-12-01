CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
PROJECT_DIR := $(realpath $(CUR_DIR)/../)
BUILD_DIR := $(PROJECT_DIR)/build

# Dependencies: need to be compiled but not to run by any Makefile target
TEST_DEPS := client data_sink
# Tests: need to be compiled and run by test-% target
TESTS := empty env hello_world malloc mmap file fs_perms getpid spawn sched pipe time \
	truncate readdir mkdir link tls pthread uname rlimit server \
	server_epoll unix_socket cout hostfs cpuid rdtsc device sleep exit_group \
	ioctl
# Benchmarks: need to be compiled and run by bench-% target
BENCHES := spawn_and_exit_latency pipe_throughput unix_socket_throughput

# Top-level Makefile targets
BUILD_TARGETS := $(TEST_DEPS) $(TESTS) $(BENCHES)
TEST_TARGETS := $(TESTS:%=test-%)
BENCH_TARGETS := $(BENCHES:%=bench-%)
.PHONY: all prebuild build postbuild test clean $(BUILD_TARGETS) $(TEST_TARGETS) $(BENCH_TARGETS)

# Use echo program instead of built-in echo command in shell. This ensures
# that echo can recognize escaped sequences (with -e argument) regardless of
# the specific shell (e.g., bash, zash, etc.)
ECHO := /bin/echo -e
# Shell escaped sequences for colorful output
CYAN := \033[1;36m
GREEN := \033[1;32m
RED := \033[1;31m
NO_COLOR := \033[0m

#############################################################################
# Build targets
#############################################################################

all: build

build: prebuild $(BUILD_TARGETS) postbuild

prebuild:
	@$(RM) -rf $(BUILD_DIR)/test
	@mkdir -p $(BUILD_DIR)/test
	@cd $(BUILD_DIR)/test && \
		$(PROJECT_DIR)/build/bin/occlum init
	@cp Occlum.json Enclave.xml $(BUILD_DIR)/test/

$(BUILD_TARGETS): %:
	@$(ECHO) "$(CYAN)BUILD TEST => $@$(NO_COLOR)"
	@$(MAKE) --no-print-directory -C $@
	@$(ECHO) "$(GREEN)DONE$(NO_COLOR)"

postbuild:
	@cd $(BUILD_DIR)/test && \
		$(PROJECT_DIR)/build/bin/occlum build

#############################################################################
# Test targets
#############################################################################

test: build $(TEST_TARGETS)

$(TEST_TARGETS): test-%: %
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

clean:
	@$(RM) -rf $(BUILD_DIR)/test
