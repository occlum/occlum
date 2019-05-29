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
BIN_FS_PATH := $(BIN_NAME)
BIN_PATH := $(FS_PATH)/$(BIN_FS_PATH)
OBJDUMP_FILE := bin.objdump
READELF_FILE := bin.readelf

CLANG_BIN_PATH := $(shell clang -print-prog-name=clang)
LLVM_PATH := $(abspath $(dir $(CLANG_BIN_PATH))../)

C_FLAGS = -Wall -O2 -fPIC $(EXTRA_C_FLAGS)
C_FLAGS += -Xclang -load -Xclang $(LLVM_PATH)/lib/LLVMMDSFIIRInserter.so
LINK_FLAGS = $(C_FLAGS) -pie -locclum_stub $(EXTRA_LINK_FLAGS)

.PHONY: all test debug clean

#############################################################################
# Build
#############################################################################

all: $(BIN_PATH)

$(BIN_PATH): $(BIN_NAME)
	@mkdir -p $(shell dirname $@)
	@cp $^ $@
	@echo "COPY => $@"

debug: $(OBJDUMP_FILE) $(READELF_FILE)

$(OBJDUMP_FILE): $(BIN_NAME)
	@objdump -d $(BIN_NAME) > $(OBJDUMP_FILE)
	@echo "OBJDUMP => $@"

$(READELF_FILE): $(BIN_NAME)
	@readelf -a -d $(BIN_NAME) > $(READELF_FILE)
	@echo "READELF => $@"

$(BIN_NAME): $(C_OBJS) $(CXX_OBJS)
	@$(CXX) $^ $(LINK_FLAGS) -o $(BIN_NAME)
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
	@cd $(CUR_DIR)/.. && RUST_BACKTRACE=1 ./pal $(BIN_FS_PATH) $(BIN_ARGS)

#############################################################################
# Misc
#############################################################################

clean:
	@-$(RM) -f *.o *.S $(BIN_NAME) $(BIN_ENC_NAME) $(OBJDUMP_FILE) $(READELF_FILE)
