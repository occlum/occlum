MAIN_MAKEFILE := $(firstword $(MAKEFILE_LIST))
INCLUDE_MAKEFILE := $(lastword $(MAKEFILE_LIST))
CUR_DIR := $(shell dirname $(realpath $(MAIN_MAKEFILE)))
PROJECT_DIR := $(realpath $(CUR_DIR)/../../)

CC := /usr/local/occlum/bin/musl-clang
CXX := /usr/local/occlum/bin/musl-clang++
C_SRCS := $(wildcard *.c)
CXX_SRCS := $(wildcard *.cc)
C_OBJS := $(C_SRCS:%.c=%.o)
CXX_OBJS := $(CXX_SRCS:%.cc=%.o)
FS_PATH := ../fs
BIN_NAME := $(shell basename $(CUR_DIR))
OBJDUMP_FILE := bin.objdump
READELF_FILE := bin.readelf

CLANG_BIN_PATH := $(shell clang -print-prog-name=clang)
LLVM_PATH := $(abspath $(dir $(CLANG_BIN_PATH))../)

C_FLAGS = -Wall -I../include -O2 -fPIC $(EXTRA_C_FLAGS)
LINK_FLAGS = $(C_FLAGS) -pie $(EXTRA_LINK_FLAGS)

.PHONY: all test debug clean

#############################################################################
# Build
#############################################################################

all: $(BIN_NAME)

# Compile C/C++ test program
#
# When compiling programs, we do not use CXX if we're not compilng any C++ files.
# This ensures C++ libraries are only linked and loaded for C++ programs, not C 
# programs.
$(BIN_NAME): $(C_OBJS) $(CXX_OBJS)
	@if [ -z $(CXX_OBJS) ] ; then \
		$(CC) $^ $(LINK_FLAGS) -o $(BIN_NAME); \
	else \
		$(CXX) $^ $(LINK_FLAGS) -o $(BIN_NAME); \
	fi ;
	@echo "LINK => $@"

$(C_OBJS): %.o: %.c
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

$(CXX_OBJS): %.o: %.cc
	@$(CXX) $(C_FLAGS) -c $< -o $@
	@echo "CXX <= $@"

#############################################################################
# Test
#############################################################################

test: $(BIN_ENC_NAME)
	@cd $(CUR_DIR)/.. && RUST_BACKTRACE=1 ./pal /bin/$(BIN_NAME) $(BIN_ARGS)

#############################################################################
# Misc
#############################################################################

clean:
	@-$(RM) -f *.o *.S $(BIN_NAME) $(BIN_ENC_NAME) $(OBJDUMP_FILE) $(READELF_FILE)
