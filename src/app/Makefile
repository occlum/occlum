include ../sgxenv.mk

APP_NAME := occlum-app

APP_A := $(OBJ_DIR)/libos/lib/lib$(APP_NAME).a

# All source code
RUST_SRCS := $(wildcard src/*.rs src/*/*.rs src/*/*/*.rs src/*/*/*/*.rs src/*/*/*/*/*.rs)
RUST_TARGET_DIR := $(OBJ_DIR)/libos/cargo-target
RUST_OUT_DIR := $(OBJ_DIR)/libos/lib
EDL_C_SRCS := $(addprefix $(OBJ_DIR)/libos/,$(SRC_OBJ)/Enclave_t.c $(SRC_OBJ)/Enclave_t.h)
EDL_C_OBJS := $(addprefix $(OBJ_DIR)/libos/,$(SRC_OBJ)/Enclave_t.o)
C_SRCS := $(sort $(wildcard src/*.c src/*/*.c src/*/*/*.c))
CXX_SRCS := $(sort $(wildcard src/*.cpp src/*/*.cpp))
S_SRCS := $(sort $(wildcard src/*.S src/*/*.S src/*/*/*.S))
C_OBJS := $(addprefix $(OBJ_DIR)/libos/,$(C_SRCS:.c=.o))
CXX_OBJS := $(addprefix $(OBJ_DIR)/libos/,$(CXX_SRCS:.cpp=.o))
S_OBJS := $(addprefix $(OBJ_DIR)/libos/,$(S_SRCS:.S=.o))

ALL_BUILD_SUBDIRS := $(sort $(patsubst %/,%,$(dir $(EDL_C_OBJS) $(C_OBJS) $(CXX_OBJS) $(S_OBJS)) $(RUST_TARGET_DIR) $(RUST_OUT_DIR)))

C_COMMON_FLAGS := -fno-stack-protector -I./include/ -I/opt/occlum/toolchains/gcc/x86_64-linux-musl/include
# SGX GDB support
C_COMMON_FLAGS += -g
C_FLAGS := $(SGX_CFLAGS_T) $(C_COMMON_FLAGS)
CXX_FLAGS := $(SGX_CXXFLAGS_T) $(C_COMMON_FLAGS)

_Other_Link_Flags := -L$(RUST_SGX_SDK_DIR)/compiler-rt/ -L$(BUILD_DIR)/lib -L$(RUST_OUT_DIR)
LINK_FLAGS := $(SGX_LFLAGS_T)

.PHONY: all clean format format-c format-rust format-check format-check-c format-check-rust

all: $(ALL_BUILD_SUBDIRS) $(APP_A)

$(ALL_BUILD_SUBDIRS):
	@mkdir -p $@

$(APP_A): $(C_OBJS) $(CXX_OBJS) $(S_OBJS)
	@ar r $@ $(C_OBJS) $(CXX_OBJS) $(S_OBJS)
	@echo "GEN => $@"

ifneq (, $(shell which sccache))
    RUSTC_WRAPPER := sccache
else
    RUSTC_WRAPPER :=
endif

$(C_OBJS):$(OBJ_DIR)/libos/$(SRC_OBJ)/%.o: src/%.c
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

$(CXX_OBJS): $(OBJ_DIR)/libos/$(SRC_OBJ)/%.o: src/%.cpp
	@$(CXX) $(CXX_FLAGS) -c $< -o $@
	@echo "CXX <= $@"

$(S_OBJS):$(OBJ_DIR)/libos/$(SRC_OBJ)/%.o: src/%.S
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "AS <= $@"

format: format-c format-rust

format-c: $(C_SRCS) $(CXX_SRCS)
	@$(C_FORMATTER) $^

format-rust: $(RUST_SRCS)
	@$(call format-rust)

format-check: format-check-c format-check-rust

format-check-c: $(C_SRCS) $(CXX_SRCS)
	@$(C_FORMATTER) --check $^

format-check-rust: $(RUST_SRCS)
	@$(call format-check-rust)

clean:
	@-$(RM) -rf $(OBJ_DIR)/libos
