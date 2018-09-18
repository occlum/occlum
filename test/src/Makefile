CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
PROJECT_DIR := $(realpath $(CUR_DIR)/../../)

C_SRCS := $(wildcard *.c)
S_FILES := $(C_SRCS:%.c=%.S)
C_OBJS := $(C_SRCS:%.c=%.o)
BIN_NAME := bin
BIN_ENC_NAME := bin.encrypted
OBJDUMP_FILE := bin.objdump
READELF_FILE := bin.readelf

C_FLAGS := -Wall -fno-builtin -fno-stack-protector -fverbose-asm -fpic
C_FLAGS += -I../include -L../rusgx_stub -lrusgx_stub
C_FLAGS += -O0
LINK_FLAGS := -pie -nostdlib -L../rusgx_stub -lrusgx_stub

.PHONY: all clean test test-without-rusgx

#############################################################################
# Build
#############################################################################

all: $(BIN_NAME) $(OBJDUMP_FILE) $(READELF_FILE)

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
#	./override_ds_with_fs.sh $@

#############################################################################
# Test
#############################################################################

test: pal librusgx.signed.so $(BIN_ENC_NAME)
	RUST_BACKTRACE=1 ./pal ./bin.encrypted

pal:
	cp $(PROJECT_DIR)/src/pal/pal pal

librusgx.signed.so:
	cp $(PROJECT_DIR)/src/libos/librusgx.signed.so librusgx.signed.so

$(BIN_ENC_NAME): $(BIN_NAME)
	$(RM) -f $(BIN_ENC_NAME)
	cd $(PROJECT_DIR)/deps/sgx_protect_file/ && \
		./sgx_protect_file encrypt -i $(CUR_DIR)/$(BIN_NAME) -o $(CUR_DIR)/$(BIN_ENC_NAME) -k 123

test-without-rusgx: $(BIN_ENC_NAME)
	LD_LIBRARY_PATH=../rusgx_stub/ ./$(BIN_NAME)

#############################################################################
# Misc
#############################################################################

clean:
	$(RM) -f *.o *.S $(BIN_NAME) $(BIN_ENC_NAME) $(OBJDUMP_FILE) $(READELF_FILE) pal librusgx.signed.so
