MAIN_MAKEFILE := $(firstword $(MAKEFILE_LIST))
INCLUDE_MAKEFILE := $(lastword $(MAKEFILE_LIST))
CUR_DIR := $(shell dirname $(realpath $(MAIN_MAKEFILE)))
PROJECT_DIR := $(realpath $(CUR_DIR)/../../)

CC := /usr/local/occlum/bin/musl-clang
C_SRCS := $(wildcard *.c)
S_FILES := $(C_SRCS:%.c=%.S)
C_OBJS := $(C_SRCS:%.c=%.o)
BIN_NAME := bin
BIN_ENC_NAME := bin.encrypted
OBJDUMP_FILE := bin.objdump
READELF_FILE := bin.readelf
FS_NAME := fs
SEFS_NAME := sefs

CLANG_BIN_PATH := $(shell clang -print-prog-name=clang)
LLVM_PATH := $(abspath $(dir $(CLANG_BIN_PATH))../)

C_FLAGS = -Wall -O0 $(EXTRA_C_FLAGS)
C_FLAGS += -Xclang -load -Xclang $(LLVM_PATH)/lib/LLVMMDSFIIRInserter.so
LINK_FLAGS = $(C_FLAGS) $(EXTRA_LINK_FLAGS)

.PHONY: all test debug clean

#############################################################################
# Build
#############################################################################

all: $(BIN_ENC_NAME) $(SEFS_NAME)

$(BIN_ENC_NAME): $(BIN_NAME)
	@$(RM) -f $(BIN_ENC_NAME)
	@cd $(PROJECT_DIR)/deps/sgx_protect_file/ && \
		./sgx_protect_file encrypt \
			-i $(CUR_DIR)/$(BIN_NAME) \
			-o $(CUR_DIR)/$(BIN_ENC_NAME) \
			-k 123 > /dev/null
	@echo "GEN => $@"

$(SEFS_NAME):
	@mkdir -p $(FS_NAME)
	@$(RM) -rf $(SEFS_NAME)
	@cd $(PROJECT_DIR)/deps/sefs/sefs-fuse/bin/ && \
		./app \
			$(CUR_DIR)/$(SEFS_NAME) \
			$(CUR_DIR)/$(FS_NAME) \
			zip
	@echo "SEFS => $@"

debug: $(OBJDUMP_FILE) $(READELF_FILE)

$(OBJDUMP_FILE): $(BIN_NAME)
	@objdump -d $(BIN_NAME) > $(OBJDUMP_FILE)
	@echo "OBJDUMP => $@"

$(READELF_FILE): $(BIN_NAME)
	@readelf -a -d $(BIN_NAME) > $(READELF_FILE)
	@echo "READELF => $@"

$(BIN_NAME): $(C_OBJS)
	@$(CC) $^ $(LINK_FLAGS) -o $(BIN_NAME)
	@echo "LINK => $@"

$(C_OBJS): %.o: %.c
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

#############################################################################
# Test
#############################################################################

test: $(BIN_ENC_NAME)
	# run test on current directory
	@ln -sf ../pal ../libocclum.signed.so $(CUR_DIR)
	@cd $(CUR_DIR) && RUST_BACKTRACE=1 ./pal $(BIN_ENC_NAME) $(BIN_ARGS)

#############################################################################
# Misc
#############################################################################

clean:
	@-$(RM) -f *.o *.S $(BIN_NAME) $(BIN_ENC_NAME) $(OBJDUMP_FILE) $(READELF_FILE)
