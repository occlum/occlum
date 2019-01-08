CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
PROJECT_DIR := $(realpath $(CUR_DIR)/../)

TEST_SUITES := empty argv hello_world malloc file getpid spawn pipe time
BUILD_TEST_SUITES := $(TEST_SUITES:%=%)
RUN_TEST_SUITES := $(TEST_SUITES:%=test-%)
CLEAN_TEST_SUITES := $(TEST_SUITES:%=clean-%)

# Use echo program instead of built-in echo command in shell. This ensures
# that echo can recognize escaped sequences (with -e argument) regardless of
# the specific shell (e.g., bash, zash, etc.)
ECHO := /bin/echo -e
# Shell escaped sequences for colorful output
CYAN := \033[1;36m
GREEN := \033[1;32m
RED := \033[1;31m
NO_COLOR := \033[0m

.PHONY: all build test clean $(BUILD_TEST_SUITES) $(RUN_TEST_SUITES) $(CLEAN_TEST_SUITES)

#############################################################################
# Build tests
#############################################################################

all: build

build: $(BUILD_TEST_SUITES)

$(BUILD_TEST_SUITES): %:
	@$(ECHO) "$(CYAN)BUILD TEST => $@$(NO_COLOR)"
	@$(MAKE) --no-print-directory -C $@
	@$(ECHO) "$(GREEN)DONE$(NO_COLOR)"

#############################################################################
# Run tests
#############################################################################

test: build $(RUN_TEST_SUITES)

pal: $(PROJECT_DIR)/src/pal/pal
	@cp $< pal

libocclum.signed.so: $(PROJECT_DIR)/src/libos/libocclum.signed.so
	@cp $< libocclum.signed.so

$(RUN_TEST_SUITES): test-%: % pal libocclum.signed.so
	@$(ECHO) "$(CYAN)RUN TEST => $<$(NO_COLOR)"
	@$(MAKE) --no-print-directory -C $< test ; \
		if [ $$? -eq 0 ] ; then \
			$(ECHO) "$(GREEN)PASS$(NO_COLOR)" ; \
		else \
			$(ECHO) "$(RED)FAILED$(NO_COLOR)" ; \
		fi ;

#############################################################################
# Misc
#############################################################################

clean: $(CLEAN_TEST_SUITES)
	@$(RM) -f pal libocclum.signed.so

$(CLEAN_TEST_SUITES): clean-%:
	@$(MAKE) --no-print-directory -C $(patsubst clean-%,%,$@) clean
