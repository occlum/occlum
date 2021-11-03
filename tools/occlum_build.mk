SGX_SDK ?= /opt/occlum/sgxsdk-tools

IMAGE := $(instance_dir)/image
SECURE_IMAGE := $(instance_dir)/build/mount/__ROOT/metadata
SECURE_IMAGE_MAC := $(instance_dir)/build/mount/.ROOT_MAC
IMAGE_CONFIG_JSON := $(instance_dir)/build/image_config.json
INITFS := $(instance_dir)/initfs
INITFS_IMAGE := $(instance_dir)/build/initfs/__ROOT/metadata
INITFS_IMAGE_MAC := $(instance_dir)/build/initfs/.ROOT_MAC
JSON_CONF := $(instance_dir)/Occlum.json
CONF_TMP_MAC := $(instance_dir)/build/tmp_mac

LIBOS := $(instance_dir)/build/lib/$(libos_lib).$(occlum_version)
SIGNED_ENCLAVE := $(instance_dir)/build/lib/libocclum-libos.signed.so

SEFS_CLI_SIM := $(occlum_dir)/build/bin/sefs-cli_sim
SIGNED_SEFS_CLI_LIB := $(occlum_dir)/build/lib/libsefs-cli.signed.so

BIN_LINKS := occlum_exec_client occlum_exec_server occlum-run
BIN_LINKS := $(addprefix $(instance_dir)/build/bin/, $(BIN_LINKS))

LIB_LINKS := libocclum-pal.so.$(major_ver) libocclum-pal.so
LIB_LINKS := $(addprefix $(instance_dir)/build/lib/, $(LIB_LINKS))

ifneq (, $(wildcard $(IMAGE)/. ))
	IMAGE_DIRS := $(shell find $(IMAGE) -type d 2>/dev/null | sed 's/ /\\ /g' | sed 's/:/\\:/g' || true)
	IMAGE_FILES := $(shell find $(IMAGE) -type f 2>/dev/null | sed 's/ /\\ /g' | sed 's/:/\\:/g' || true)
endif

ifneq (, $(wildcard $(INITFS)/. ))
	INITFS_DIRS := $(shell find $(INITFS) -type d 2>/dev/null | sed 's/ /\\ /g' | sed 's/:/\\:/g' || true)
	INITFS_FILES := $(shell find $(INITFS) -type f 2>/dev/null | sed 's/ /\\ /g' | sed 's/:/\\:/g' || true)
endif

SHELL:=/bin/bash

define get_occlum_file_mac
	LD_LIBRARY_PATH="$(SGX_SDK)/sdk_libs" \
		"$(occlum_dir)/build/bin/occlum-protect-integrity" show-mac $(1) $(2)
endef

.PHONY : all clean

ALL_TARGETS := $(SIGNED_ENCLAVE) $(BIN_LINKS) $(LIB_LINKS)

all: $(ALL_TARGETS)

$(SIGNED_ENCLAVE): $(LIBOS)
	@echo "Signing the enclave..."

	@$(ENCLAVE_SIGN_TOOL) sign \
		-key $(ENCLAVE_SIGN_KEY) \
		-config "$(instance_dir)/build/Enclave.xml" \
		-enclave "$(instance_dir)/build/lib/libocclum-libos.so.$(major_ver)" \
		-out "$(instance_dir)/build/lib/libocclum-libos.signed.so"

$(LIBOS): $(instance_dir)/build/.Occlum_sys.json.protected
	@echo "Building libOS..."
	@cd $(instance_dir)/build/lib && \
		cp "$(occlum_dir)/build/lib/$(libos_lib).$(occlum_version)" . && \
		ln -sf "$(libos_lib).$(occlum_version)" "libocclum-libos.so.$(major_ver)" && \
		ln -sf "libocclum-libos.so.$(major_ver)" libocclum-libos.so ; \
		$(call get_occlum_file_mac, "$(instance_dir)/build/.Occlum_sys.json.protected", "$(CONF_TMP_MAC)") && \
		objcopy --update-section .builtin_config="$(CONF_TMP_MAC)" libocclum-libos.so && \
		rm -f "$(CONF_TMP_MAC)"

$(instance_dir)/build/.Occlum_sys.json.protected: $(instance_dir)/build/.Occlum_sys.json
	@cd "$(instance_dir)/build" ; \
		LD_LIBRARY_PATH="$(SGX_SDK)/sdk_libs" "$(occlum_dir)/build/bin/occlum-protect-integrity" protect .Occlum_sys.json ;

$(instance_dir)/build/.Occlum_sys.json: $(INITFS_IMAGE) $(INITFS_IMAGE_MAC) $(JSON_CONF)
	@$(occlum_dir)/build/bin/gen_internal_conf --user_json "$(JSON_CONF)" gen_sys_conf \
		--init_fs_mac "`cat $(INITFS_IMAGE_MAC)`" --sys_json $(instance_dir)/build/.Occlum_sys.json

$(BIN_LINKS): $(instance_dir)/build/bin/%: $(occlum_dir)/build/bin/% | $(instance_dir)/build/bin
	@ln -sf $< $@

$(instance_dir)/build/bin:
	@mkdir -p build/bin

$(instance_dir)/build/lib/libocclum-pal.so:
$(instance_dir)/build/lib/libocclum-pal.so.0: | $(instance_dir)/build/lib
	@cp "$(occlum_dir)/build/lib/$(pal_lib).$(occlum_version)" build/lib/
	@cd build/lib && ln -sf "$(pal_lib).$(occlum_version)" "libocclum-pal.so.$(major_ver)" && \
		ln -sf "libocclum-pal.so.$(major_ver)" libocclum-pal.so

$(instance_dir)/build/lib:
	@mkdir -p build/lib

$(INITFS_IMAGE_MAC):
$(INITFS_IMAGE): $(INITFS) $(INITFS_DIRS) $(INITFS_FILES) $(IMAGE_CONFIG_JSON) $(SEFS_CLI_SIM) $(SIGNED_SEFS_CLI_LIB)
	@echo "Building the initfs..."
	@rm -rf build/initfs
	@mkdir -p build/initfs
	@[ "$(BUILDIN_IMAGE_KEY)" == "true" ] && \
		cp "$(SECURE_IMAGE_KEY)" "$(INITFS)/etc/image_key" || \
		rm -f "$(INITFS)/etc/image_key"
	@cp "$(IMAGE_CONFIG_JSON)" "$(INITFS)/etc/"
	@LD_LIBRARY_PATH="$(SGX_SDK)/sdk_libs" $(SEFS_CLI_SIM) \
		--enclave "$(SIGNED_SEFS_CLI_LIB)" \
		zip \
		"$(INITFS)" \
		"$(instance_dir)/build/initfs/__ROOT" \
		"$(INITFS_IMAGE_MAC)"

$(IMAGE_CONFIG_JSON): $(instance_dir)/build/Occlum.json.protected
	@$(call get_occlum_file_mac, "$(instance_dir)/build/Occlum.json.protected", "$(CONF_TMP_MAC)") && \
		[ -n "$(SECURE_IMAGE_KEY)" ] && \
		jq -n --arg mac_val "`cat $(CONF_TMP_MAC)`" \
		'{image_type: "encrypted", occlum_json_mac: $$mac_val}' > $(IMAGE_CONFIG_JSON) || \
		jq -n --arg mac_val "`cat $(CONF_TMP_MAC)`" \
		'{image_type: "integrity-only", occlum_json_mac: $$mac_val}' > $(IMAGE_CONFIG_JSON)
	@rm -f "$(CONF_TMP_MAC)"

$(instance_dir)/build/Occlum.json.protected: $(instance_dir)/build/Occlum.json
	@cd "$(instance_dir)/build" ; \
		LD_LIBRARY_PATH="$(SGX_SDK)/sdk_libs" "$(occlum_dir)/build/bin/occlum-protect-integrity" protect Occlum.json ;

$(instance_dir)/build/Enclave.xml:
$(instance_dir)/build/Occlum.json: $(SECURE_IMAGE) $(SECURE_IMAGE_MAC) $(JSON_CONF) | $(instance_dir)/build/lib
	@$(occlum_dir)/build/bin/gen_internal_conf --user_json "$(JSON_CONF)" gen_user_conf \
		--user_fs_mac "`cat $(SECURE_IMAGE_MAC)`" --sdk_xml "$(instance_dir)/build/Enclave.xml"  \
		--output_user_json $(instance_dir)/build/Occlum.json

# If image dir not exist, just use the secure Occlum FS image
ifneq ($(wildcard $(IMAGE)/. ),)
$(SECURE_IMAGE_MAC):
$(SECURE_IMAGE): $(IMAGE) $(IMAGE_DIRS) $(IMAGE_FILES) $(SEFS_CLI_SIM) $(SIGNED_SEFS_CLI_LIB)
	@echo "Building new image..."
	@rm -rf build/mount
	@mkdir -p build/mount/
	@[ -n "$(SECURE_IMAGE_KEY)" ] && export SECURE_IMAGE_KEY_OPTION="--key $(SECURE_IMAGE_KEY)" ; \
		LD_LIBRARY_PATH="$(SGX_SDK)/sdk_libs" $(SEFS_CLI_SIM) \
			--enclave "$(SIGNED_SEFS_CLI_LIB)" \
			zip \
			$$SECURE_IMAGE_KEY_OPTION \
			"$(IMAGE)" \
			"$(instance_dir)/build/mount/__ROOT" \
			"$(SECURE_IMAGE_MAC)"
endif

clean:
	rm -rf $(instance_dir)/build
