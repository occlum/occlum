######## SGX SDK Settings ########

CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
PROJECT_DIR := $(realpath $(CUR_DIR)/../../)

MAJOR_VER_NUM = $(shell grep '\#define OCCLUM_MAJOR_VERSION' $(PROJECT_DIR)/src/pal/include/occlum_version.h |  awk '{print $$3}')
MINOR_VER_NUM = $(shell grep '\#define OCCLUM_MINOR_VERSION' $(PROJECT_DIR)/src/pal/include/occlum_version.h |  awk '{print $$3}')
PATCH_VER_NUM = $(shell grep '\#define OCCLUM_PATCH_VERSION' $(PROJECT_DIR)/src/pal/include/occlum_version.h |  awk '{print $$3}')
VERSION_NUM = $(MAJOR_VER_NUM).$(MINOR_VER_NUM).$(PATCH_VER_NUM)

SGX_SDK ?= /opt/intel/sgxsdk
SGX_MODE ?= HW
SGX_ARCH ?= x64
SGX_DEBUG ?= 1

C_FORMATTER := $(PROJECT_DIR)/tools/c_formatter

BUILD_DIR := $(PROJECT_DIR)/build
OBJS_DIR := $(BUILD_DIR)/internal/tools/protect-integrity

ifeq ($(shell getconf LONG_BIT), 32)
	SGX_ARCH := x86
else ifeq ($(findstring -m32, $(CXXFLAGS)), -m32)
	SGX_ARCH := x86
endif

ifeq ($(SGX_ARCH), x86)
	SGX_COMMON_CFLAGS := -m32
	SGX_LIBRARY_PATH := $(SGX_SDK)/lib
	SGX_ENCLAVE_SIGNER := $(SGX_SDK)/bin/x86/sgx_sign
	SGX_EDGER8R := $(SGX_SDK)/bin/x86/sgx_edger8r
else
	SGX_COMMON_CFLAGS := -m64
	SGX_LIBRARY_PATH := $(SGX_SDK)/lib64
	SGX_ENCLAVE_SIGNER := $(SGX_SDK)/bin/x64/sgx_sign
	SGX_EDGER8R := $(SGX_SDK)/bin/x64/sgx_edger8r
endif

ifeq ($(SGX_DEBUG), 1)
ifeq ($(SGX_PRERELEASE), 1)
$(error Cannot set SGX_DEBUG and SGX_PRERELEASE at the same time!!)
endif
endif

SGX_COMMON_CFLAGS += -Wall -Wno-unused-result

ifeq ($(SGX_DEBUG), 1)
	SGX_COMMON_CFLAGS += -O2 -g
else
	SGX_COMMON_CFLAGS += -O2
endif

######## App Settings ########

ifneq ($(SGX_MODE), HW)
	# Tools are built in simulation mode by default and should be built with sgx_urts_sim_with_se_event
	# to resolve undefined symbols
	URTS_LIBRARY_NAME := sgx_urts_sim_with_se_event
else
	URTS_LIBRARY_NAME := sgx_urts
endif

APP_C_FILES := App/Enclave_u.c
APP_CPP_FILES := App/App.cpp
APP_HEADER_FILES := App/App.h
APP_INCLUDE_PATHS := -IInclude -IApp -I$(SGX_SDK)/include -I$(OBJS_DIR)/App

APP_C_FLAGS := $(SGX_COMMON_CFLAGS) -fPIC -Wno-attributes $(APP_INCLUDE_PATHS)

# Three configuration modes - Debug, prerelease, release
#   Debug - Macro DEBUG enabled.
#   Prerelease - Macro NDEBUG and EDEBUG enabled.
#   Release - Macro NDEBUG enabled.
ifeq ($(SGX_DEBUG), 1)
	APP_C_FLAGS += -DDEBUG -UNDEBUG -UEDEBUG
else ifeq ($(SGX_PRERELEASE), 1)
	APP_C_FLAGS += -DNDEBUG -DEDEBUG -UDEBUG
else
	APP_C_FLAGS += -DNDEBUG -UEDEBUG -UDEBUG
endif

APP_CPP_FLAGS := $(APP_C_FLAGS) -std=c++11
APP_LINK_FLAGS := $(SGX_COMMON_CFLAGS) -lpthread -L$(SGX_LIBRARY_PATH) -Wl,-Bstatic -l$(URTS_LIBRARY_NAME) -Wl,-Bdynamic

APP_LINK_FLAGS += -lsgx_uprotected_fs -ldl -lcrypto

ifneq ($(SGX_MODE), HW)
	APP_LINK_FLAGS += -lsgx_uae_service_sim
else
	APP_LINK_FLAGS += -lsgx_uae_service
endif

APP_C_OBJS := $(addprefix $(OBJS_DIR)/,$(APP_C_FILES:.c=.o))
APP_CPP_OBJS := $(addprefix $(OBJS_DIR)/,$(APP_CPP_FILES:.cpp=.o))
APP_OBJS := $(APP_C_OBJS) $(APP_CPP_OBJS)

APP_NAME := $(BUILD_DIR)/bin/occlum-protect-integrity

######## Enclave Settings ########

ifneq ($(SGX_MODE), HW)
	TRTS_LIBRARY_NAME := sgx_trts_sim
	SERVICE_LIBRARY_NAME := sgx_tservice_sim
else
	TRTS_LIBRARY_NAME := sgx_trts
	SERVICE_LIBRARY_NAME := sgx_tservice
endif
CRYPTO_LIBRARY_NAME := sgx_tcrypto

ENCLAVE_C_FILES := Enclave/Enclave_t.c
ENCLAVE_CPP_FILES := Enclave/Enclave.cpp
ENCLAVE_HEADER_FILES := Enclave/Enclave.h
ENCLAVE_INCLUDE_PATHS := -IInclude -IEnclave -I$(SGX_SDK)/include -I$(SGX_SDK)/include/tlibc -I$(SGX_SDK)/include/stlport -I$(OBJS_DIR)/Enclave

ENCLAVE_C_FLAGS := $(SGX_COMMON_CFLAGS) -nostdinc -fvisibility=hidden -fpie -fPIC -fstack-protector $(ENCLAVE_INCLUDE_PATHS)
ENCLAVE_CPP_FLAGS := $(ENCLAVE_C_FLAGS) -std=c++03 -nostdinc++

# To generate a proper enclave, it is recommended to follow below guideline to link the trusted libraries:
#    1. Link sgx_trts with the `--whole-archive' and `--no-whole-archive' options,
#       so that the whole content of trts is included in the enclave.
#    2. For other libraries, you just need to pull the required symbols.
#       Use `--start-group' and `--end-group' to link these libraries.
# Do NOT move the libraries linked with `--start-group' and `--end-group' within `--whole-archive' and `--no-whole-archive' options.
# Otherwise, you may get some undesirable errors.
ENCLAVE_LINK_FLAGS := $(SGX_COMMON_CFLAGS) -Wl,--no-undefined -nostdlib -nodefaultlibs -nostartfiles -L$(SGX_LIBRARY_PATH) \
	-Wl,--whole-archive -l$(TRTS_LIBRARY_NAME) -Wl,--no-whole-archive \
	-Wl,--start-group -lsgx_tstdc -lsgx_tcxx -lsgx_tprotected_fs -l$(CRYPTO_LIBRARY_NAME) -l$(SERVICE_LIBRARY_NAME) -Wl,--end-group \
	-Wl,-Bstatic -Wl,-Bsymbolic -Wl,--no-undefined \
	-Wl,-pie,-eenclave_entry -Wl,--export-dynamic  \
	-Wl,--defsym,__ImageBase=0 \
	-Wl,--version-script=Enclave/Enclave.lds

ENCLAVE_C_OBJS := $(addprefix $(OBJS_DIR)/,$(ENCLAVE_C_FILES:.c=.o))
ENCLAVE_CPP_OBJS := $(addprefix $(OBJS_DIR)/,$(ENCLAVE_CPP_FILES:.cpp=.o))
ENCLAVE_OBJS := $(ENCLAVE_C_OBJS) $(ENCLAVE_CPP_OBJS)

ENCLAVE_NAME := $(BUILD_DIR)/lib/occlum-protect-integrity.so
ENCLAVE_REAL_NAME := $(ENCLAVE_NAME).$(VERSION_NUM)
SIGNED_ENCLAVE_NAME := $(BUILD_DIR)/lib/occlum-protect-integrity.signed.so.$(VERSION_NUM)
SIGNED_ENCLAVE_SONAME := $(BUILD_DIR)/lib/occlum-protect-integrity.signed.so.$(MAJOR_VER_NUM) # used for soft link
ENCLAVE_CONFIG_FILE := Enclave/Enclave.config.xml

ALL_BUILD_SUBDIRS := $(sort $(patsubst %/,%,$(dir $(APP_NAME) $(SIGNED_ENCLAVE_NAME) $(ENCLAVE_OBJS) $(APP_OBJS))))

ifeq ($(SGX_MODE), HW)
ifneq ($(SGX_DEBUG), 1)
ifneq ($(SGX_PRERELEASE), 1)
BUILD_MODE = HW_RELEASE
endif
endif
endif


.PHONY: all test format format-check clean

ifeq ($(BUILD_MODE), HW_RELEASE)
all: $(APP_NAME) $(ENCLAVE_REAL_NAME)
	@echo "The project has been built in release hardware mode."
	@echo "Please sign the $(ENCLAVE_REAL_NAME) first with your signing key before you run the $(APP_NAME) to launch and access the enclave."
	@echo "To sign the enclave use the command:"
	@echo "   $(SGX_ENCLAVE_SIGNER) sign -key <your key> -enclave $(ENCLAVE_REAL_NAME) -out <$(SIGNED_ENCLAVE_NAME)> -config $(ENCLAVE_CONFIG_FILE)"
	@echo "You can also sign the enclave using an external signing tool."
	@echo "To build the project in simulation mode set SGX_MODE=SIM. To build the project in prerelease mode set SGX_PRERELEASE=1 and SGX_MODE=HW."
else

all: $(ALL_BUILD_SUBDIRS) $(APP_NAME) $(SIGNED_ENCLAVE_SONAME)

endif

$(ALL_BUILD_SUBDIRS):
	@mkdir -p $@

######## App Objects ########

$(OBJS_DIR)/App/Enclave_u.c: $(SGX_EDGER8R) Enclave.edl
	@cd $(OBJS_DIR)/App && $(SGX_EDGER8R) --untrusted $(CUR_DIR)/Enclave.edl --search-path $(SGX_SDK)/include
	@echo "GEN <=  $@"

$(OBJS_DIR)/App/Enclave_u.o: $(OBJS_DIR)/App/Enclave_u.c
	@$(CC) $(APP_C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

$(OBJS_DIR)/App/%.o: App/%.cpp
	@$(CXX) $(APP_CPP_FLAGS) -c $< -o $@
	@echo "CXX <= $@"

$(OBJS_DIR)/App/%.o: App/%.c
	@$(CC) $(APP_C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

$(APP_NAME): $(APP_OBJS)
	@$(CXX) $^ -o $@ $(APP_LINK_FLAGS)
	@echo "LINK =>  $@"


######## Enclave Objects ########

$(OBJS_DIR)/Enclave/Enclave_t.c: $(SGX_EDGER8R) Enclave.edl
	@cd $(OBJS_DIR)/Enclave && $(SGX_EDGER8R) --trusted $(CUR_DIR)/Enclave.edl --search-path $(SGX_SDK)/include
	@echo "GEN <=  $@"

$(OBJS_DIR)/Enclave/Enclave_t.o: $(OBJS_DIR)/Enclave/Enclave_t.c
	@$(CXX) $(ENCLAVE_CPP_FLAGS) -c $< -o $@
	@echo "CXX <=  $@"

$(OBJS_DIR)/Enclave/%.o: Enclave/%.cpp
	@$(CXX) $(ENCLAVE_CPP_FLAGS) -c $< -o $@
	@echo "CXX <=  $@"

$(OBJS_DIR)/Enclave/%.o: Enclave/%.c
	@$(CC) $(ENCLAVE_C_FLAGS) -c $< -o $@
	@echo "CC  <=  $@"

$(ENCLAVE_REAL_NAME): $(ENCLAVE_OBJS)
	@$(CXX) $^ -o $@ $(ENCLAVE_LINK_FLAGS)
	@echo "LINK =>  $@"

$(SIGNED_ENCLAVE_SONAME): $(ENCLAVE_REAL_NAME)
	@$(SGX_ENCLAVE_SIGNER) sign -key Enclave/Enclave_private.pem -enclave $(ENCLAVE_REAL_NAME) -out $(SIGNED_ENCLAVE_NAME) -config $(ENCLAVE_CONFIG_FILE)
	@cd $(BUILD_DIR)/lib && ln -sf $(notdir $(SIGNED_ENCLAVE_NAME)) $(notdir $(SIGNED_ENCLAVE_SONAME))
	@echo "SIGN =>  $@"

test: all random.txt
	$(APP_NAME) protect random.txt
	$(APP_NAME) show random.txt.protected > random.txt.unprotected
	$(APP_NAME) show-mac random.txt.protected
	diff random.txt random.txt.unprotected
	@echo "Pass ^_^"

random.txt:
	@base64 /dev/urandom | head -c 10000000 > random.txt


format: $(APP_HEADER_FILES) $(APP_CPP_FILES) $(ENCLAVE_HEADER_FILES) $(ENCLAVE_CPP_FILES)
	@$(C_FORMATTER) $^

format-check: $(APP_HEADER_FILES) $(APP_CPP_FILES) $(ENCLAVE_HEADER_FILES) $(ENCLAVE_CPP_FILES)
	@$(C_FORMATTER) --check $^


clean:
	@rm -f $(APP_NAME) $(ENCLAVE_REAL_NAME) $(SIGNED_ENCLAVE_NAME) $(SIGNED_ENCLAVE_SONAME) $(APP_OBJS) $(OBJS_DIR)/App/Enclave_u.* $(ENCLAVE_OBJS) $(OBJS_DIR)/Enclave/Enclave_t.* *.test.txt random.txt*
