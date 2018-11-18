MAIN_MAKEFILE := $(firstword $(MAKEFILE_LIST))
INCLUDE_MAKEFILE := $(lastword $(MAKEFILE_LIST))
CUR_DIR := $(shell dirname $(realpath $(MAIN_MAKEFILE)))
PROJECT_DIR := $(realpath $(CUR_DIR)/../../)

CC := /usr/local/occlum/bin/musl-gcc
C_SRCS := $(wildcard *.c)
S_FILES := $(C_SRCS:%.c=%.S)
C_OBJS := $(C_SRCS:%.c=%.o)
BIN_NAME := bin
BIN_ENC_NAME := bin.encrypted
OBJDUMP_FILE := bin.objdump
READELF_FILE := bin.readelf

C_FLAGS = -Wall -fverbose-asm -O0 $(EXTRA_C_FLAGS)
LINK_FLAGS = $(EXTRA_LINK_FLAGS)

.PHONY: all run debug clean

#############################################################################
# Build
#############################################################################

all: $(BIN_ENC_NAME)

$(BIN_ENC_NAME): $(BIN_NAME)
	$(RM) -f $(BIN_ENC_NAME)
	cd $(PROJECT_DIR)/deps/sgx_protect_file/ && \
		./sgx_protect_file encrypt -i $(CUR_DIR)/$(BIN_NAME) -o $(CUR_DIR)/$(BIN_ENC_NAME) -k 123

debug: $(OBJDUMP_FILE) $(READELF_FILE)

$(OBJDUMP_FILE): $(BIN_NAME)
	objdump -d $(BIN_NAME) > $(OBJDUMP_FILE)

$(READELF_FILE): $(BIN_NAME)
	readelf -a -d $(BIN_NAME) > $(READELF_FILE)

$(BIN_NAME): $(C_OBJS)
	$(CC) $^ $(LINK_FLAGS) -o $(BIN_NAME)

$(C_OBJS): %.o: %.S
	$(CC) $(C_FLAGS) -c $< -o $@

$(S_FILES): %.S: %.c
	$(CC) $(C_FLAGS) -S $< -o $@

#############################################################################
# Test
#############################################################################

run: $(BIN_ENC_NAME)
	cd ../ && RUST_BACKTRACE=1 ./pal $(CUR_DIR)/$(BIN_ENC_NAME)

#############################################################################
# Misc
#############################################################################

clean:
	$(RM) -f *.o *.S $(BIN_NAME) $(BIN_ENC_NAME) $(OBJDUMP_FILE) $(READELF_FILE)
