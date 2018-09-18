C_SRCS := $(wildcard *.c)
S_FILES := $(C_SRCS:%.c=%.S)
C_OBJS := $(C_SRCS:%.c=%.o)
LIB_NAME := librusgx_stub.so

C_FLAGS := -Wall -fno-builtin -fno-stack-protector -fverbose-asm -fpic -I../include
C_FLAGS += -O0
LINK_FLAGS := -nostdlib

.PHONY: all clean
all: $(LIB_NAME)

$(LIB_NAME): $(C_OBJS)
	$(CC) $^ $(LINK_FLAGS) -shared -o $(LIB_NAME)

$(C_OBJS): %.o: %.S
	$(CC) $(C_FLAGS) -c $< -o $@

$(S_FILES): %.S: %.c
	$(CC) $(C_FLAGS) -S $< -o $@

clean:
	$(RM) -f *.o *.S $(LIB_NAME)
