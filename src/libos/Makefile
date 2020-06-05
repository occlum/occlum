include ../sgxenv.mk

# An overview of the build process
#
# The target library:
#     libocclum-libos.so
#
# The intermediate libraries:
#     libocclum-libos-core.a
#     libocclum_libos_core_rs.a
#
# Diagram:
#
# +-------------------+
# |LibOS (Enclave)<--------+ libocclum-libos.so
# | +---------------| |
# | |LibOS Core <----------+ libocclum-libos-core.a
# | |  +----------| | |
# | |  |Rust <-------------+ libocclum_libos_core_rs.a
# | |  +----------+ | |      (Rust forbids the use of hypens in library names)
# | |  |C         | | |
# | |  +----------+ | |
# | |  |Assembly  | | |
# | |  +----------+ | |
# | +---------------+ |
# | |LibOS Builtins<-------+ Configurable by Occlum.json
# | +---------------+ |
# |                   |
# | +---------------+ |
# | |Rust SGX SDK <-----+
# | +---------------| | +--+ Dependencies
# | |Intel SGX SDK<-----+
# | +---------------| |
# +-------------------+

# The log level for LibOS
#
# There are five levels:
# 1 - error
# 2 - warn
# 3 - info
# 4 - debug
# 5 - trace
#
# By setting the log level to a specific value (say warn), all log messages
# whose levels are no greater than the value (error and warn <= warn) will
# be printed.
LIBOS_LOG ?= error

ONLY_REBUILD_BUILTIN ?= 0

LIBOS_SO := $(BUILD_DIR)/lib/libocclum-libos.so
LIBOS_CORE_A := $(BUILD_DIR)/lib/libocclum-libos-core.a
LIBOS_CORE_RS_A := $(BUILD_DIR)/lib/libocclum_libos_core_rs.a

# All source code
RUST_SRCS := $(wildcard src/*.rs src/*/*.rs src/*/*/*.rs src/*/*/*/*.rs src/*/*/*/*/*.rs)
RUST_TARGET_DIR := $(BUILD_DIR)/src/libos/cargo-target
RUST_OUT_DIR := $(BUILD_DIR)/lib
EDL_C_SRCS := $(addprefix $(BUILD_DIR)/src/libos/,src/Enclave_t.c src/Enclave_t.h)
EDL_C_OBJS := $(addprefix $(BUILD_DIR)/src/libos/,src/Enclave_t.o)
BUILTIN_C_SRCS := $(sort $(wildcard src/builtin/*.c))
BUILTIN_C_OBJS := $(addprefix $(BUILD_DIR)/src/libos/,$(BUILTIN_C_SRCS:.c=.o))
C_SRCS := $(filter-out $(BUILTIN_C_SRCS),$(sort $(wildcard src/*.c src/*/*.c src/*/*/*.c)))
C_OBJS := $(addprefix $(BUILD_DIR)/src/libos/,$(C_SRCS:.c=.o))
CXX_SRCS := $(sort $(wildcard src/*.cpp src/*/*.cpp))
CXX_OBJS := $(addprefix $(BUILD_DIR)/src/libos/,$(CXX_SRCS:.cpp=.o))
S_SRCS := $(sort $(wildcard src/*.S src/*/*.S src/*/*/*.S))
S_OBJS := $(addprefix $(BUILD_DIR)/src/libos/,$(S_SRCS:.S=.o))

ALL_BUILD_SUBDIRS := $(sort $(patsubst %/,%,$(dir $(LIBOS_SO) $(EDL_C_OBJS) $(BUILTIN_C_OBJS) $(C_OBJS) $(CXX_OBJS) $(S_OBJS)) $(RUST_TARGET_DIR) $(RUST_OUT_DIR)))

C_COMMON_FLAGS := -fno-stack-protector -I./include/
# SGX GDB support
C_COMMON_FLAGS += -g
C_FLAGS := $(SGX_CFLAGS_T) $(C_COMMON_FLAGS)
CXX_FLAGS := $(SGX_CXXFLAGS_T) $(C_COMMON_FLAGS)

# Pass builtin values by defining macros
#
# The MAC of Occlum config file must be builtin into the binary
ifdef OCCLUM_BUILTIN_CONF_FILE_MAC
C_FLAGS += -DOCCLUM_BUILTIN_CONF_FILE_MAC='"$(OCCLUM_BUILTIN_CONF_FILE_MAC)"'
endif

_Other_Link_Flags := -L$(RUST_SGX_SDK_DIR)/compiler-rt/ -L$(BUILD_DIR)/lib
_Other_Enclave_Libs := -locclum-libos-core -lsgx_tprotected_fs
LINK_FLAGS := $(SGX_LFLAGS_T)

.PHONY: all clean

all: $(ALL_BUILD_SUBDIRS) $(LIBOS_SO)

$(ALL_BUILD_SUBDIRS):
	@mkdir -p $@

LIBOS_SO_DEPS := $(BUILTIN_C_OBJS) $(LIBOS_CORE_A) $(LIBCOMPILER_RT_PATCH_A)
ifeq ($(ONLY_REBUILD_BUILTIN), 0)
$(LIBOS_SO): $(BUILTIN_C_OBJS) $(LIBOS_CORE_A) $(LIBCOMPILER_RT_PATCH_A)
	@$(CC) $(LIBOS_SO_DEPS) -o $@ $(LINK_FLAGS)
	@echo "LINK => $@"
else
$(LIBOS_SO): $(BUILTIN_C_OBJS)
	@$(CC) $(LIBOS_SO_DEPS) -o $@ $(LINK_FLAGS)
	@echo "LINK => $@"
endif

$(LIBOS_CORE_A): $(LIBOS_CORE_RS_A) $(C_OBJS) $(CXX_OBJS) $(S_OBJS) $(EDL_C_OBJS)
	@cp $(LIBOS_CORE_RS_A) $(LIBOS_CORE_A)
	@ar r $@ $(C_OBJS) $(CXX_OBJS) $(S_OBJS) $(EDL_C_OBJS)
	@echo "GEN => $@"

ifeq ($(OCCLUM_RELEASE_BUILD), 1)
$(LIBOS_CORE_RS_A): $(RUST_SRCS)
	@RUSTC_BOOTSTRAP=1 cargo build --release --target-dir=$(RUST_TARGET_DIR) -Z unstable-options --out-dir=$(RUST_OUT_DIR)
	@echo "CARGO (release) => $@"
else
$(LIBOS_CORE_RS_A): $(RUST_SRCS)
	@RUSTC_BOOTSTRAP=1 cargo build --target-dir=$(RUST_TARGET_DIR) -Z unstable-options --out-dir=$(RUST_OUT_DIR)
	@echo "CARGO (debug) => $@"
endif

$(BUILD_DIR)/src/libos/src/Enclave_t.o: $(BUILD_DIR)/src/libos/src/Enclave_t.c
	@$(CC) $(C_FLAGS) -Iinclude/edl -c $< -o $@
	@echo "CC <= $@"

$(BUILD_DIR)/src/libos/src/Enclave_t.c: $(SGX_EDGER8R) ../Enclave.edl
	@cd $(BUILD_DIR)/src/libos/src && $(SGX_EDGER8R) --trusted $(CUR_DIR)/../Enclave.edl --search-path $(SGX_SDK)/include --search-path $(RUST_SGX_SDK_DIR)/edl
	@echo "GEN <= $@"

$(BUILD_DIR)/src/libos/%.o: %.c
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

$(BUILD_DIR)/src/libos/%.o: %.cpp
	@$(CXX) $(CXX_FLAGS) -c $< -o $@
	@echo "CXX <= $@"

$(BUILD_DIR)/src/libos/%.o: %.S
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "AS <= $@"

clean-builtin:
	@-$(RM) -f $(BUILTIN_C_OBJS)

clean: clean-builtin
	@-$(RM) -f $(LIBOS_SO) $(LIBOS_CORE_A) $(LIBOS_CORE_RS_A) \
		$(EDL_C_OBJS) $(EDL_C_SRCS) $(C_OBJS) $(CXX_OBJS) $(S_OBJS)
	@-$(RM) -rf $(RUST_TARGET_DIR)
