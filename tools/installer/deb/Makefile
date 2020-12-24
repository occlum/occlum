CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
PROJECT_DIR := $(CUR_DIR)/../../..
TARGET_DIR := $(abspath $(PROJECT_DIR)/build/debs)

MAJOR_VER_NUM = $(shell grep '\#define OCCLUM_MAJOR_VERSION' $(PROJECT_DIR)/src/pal/include/occlum_version.h | awk '{print $$3}')
MINOR_VER_NUM = $(shell grep '\#define OCCLUM_MINOR_VERSION' $(PROJECT_DIR)/src/pal/include/occlum_version.h | awk '{print $$3}')
PATCH_VER_NUM = $(shell grep '\#define OCCLUM_PATCH_VERSION' $(PROJECT_DIR)/src/pal/include/occlum_version.h | awk '{print $$3}')
VERSION_NUM = $(MAJOR_VER_NUM).$(MINOR_VER_NUM).$(PATCH_VER_NUM)

DEB_BUILD_DIR := /tmp/deb_build
PACKAGE_DIR := /tmp

C_TOOLCHAIN_PACKAGE_NAME := occlum-toolchains-gcc

# Version number for toolchain packages
# The version numbers are only needed to update when the toolchains are updated.
# When updating the toolchain packages for Deb, please first update RPM configuration. And then
# update the "changelog" file accordingly.
MUSL_VERSION := 0.15.0
GOLAN_VERSION := 0.16.0

GO_PATCH = $(PROJECT_DIR)/tools/toolchains/golang/adapt-golang-to-occlum.patch

.PHONY: all main config_files main_deps musl-gcc deps_c clean clean-build

all: main

main: config_files
	cd $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM); \
		export VERSION=$(VERSION_NUM); dpkg-buildpackage -us -uc
	cp -t $(TARGET_DIR) $(DEB_BUILD_DIR)/occlum-runtime*.deb $(DEB_BUILD_DIR)/occlum-runtime-dbgsym*.ddeb $(DEB_BUILD_DIR)/occlum-pal*.deb \
		$(DEB_BUILD_DIR)/occlum-pal-dbgsym*.ddeb $(DEB_BUILD_DIR)/occlum_$(VERSION_NUM)*.deb $(DEB_BUILD_DIR)/occlum-dbgsym_$(VERSION_NUM)*.ddeb \
		$(DEB_BUILD_DIR)/occlum-sgx-tools*.deb $(DEB_BUILD_DIR)/occlum-sgx-tools-dbgsym*.ddeb

config_files: main_deps
	echo " echo 'Please execute command \"source /etc/profile\" to validate envs immediately' " > $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/debian/occlum-pal.postinst
	cp $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/debian/occlum-pal.postinst $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/debian/occlum-runtime.postinst
	cp $(PROJECT_DIR)/tools/installer/rpm/occlum*filelist $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/debian
	cp $(PROJECT_DIR)/tools/installer/rpm/occlum-*.sh $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/debian
	sed -e 's/^\///g' -i $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/debian/occlum*filelist
	sed -i -e '$$aetc/profile.d/occlum-pal.sh' $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/debian/occlum-pal-filelist
	sed -i -e '$$aetc/profile.d/occlum-runtime.sh' $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/debian/occlum-runtime-filelist
	sed -i -e 's/$$OCCLUM_VERSION/$(VERSION_NUM)/g' $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/debian/changelog
	sed -i -e 's/$$OCCLUM_VERSION/$(VERSION_NUM)/g' $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/debian/control
	sed -i -e 's/$$MUSL_VERSION/$(MUSL_VERSION)/g' $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/debian/control

main_deps:
	mkdir -p $(TARGET_DIR)
	mkdir -p $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)
	cp -r occlum $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/debian
	@cp -r $(PROJECT_DIR) $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/occlum-src
	@cd $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/occlum-src; \
		git submodule deinit -f . ; \
		rm -rf $(DEB_BUILD_DIR)/occlum-$(VERSION_NUM)/occlum-src/{build,demos}

musl-gcc: deps_c
	cd $(DEB_BUILD_DIR)/$(C_TOOLCHAIN_PACKAGE_NAME) ; \
		dpkg-buildpackage -us -uc
	cp -t $(TARGET_DIR) $(DEB_BUILD_DIR)/$(C_TOOLCHAIN_PACKAGE_NAME)*.deb $(DEB_BUILD_DIR)/$(C_TOOLCHAIN_PACKAGE_NAME)-dbgsym*.ddeb

deps_c:
	mkdir -p $(TARGET_DIR)
	mkdir -p $(DEB_BUILD_DIR)/$(C_TOOLCHAIN_PACKAGE_NAME)
	cp -r toolchains/musl-gcc $(DEB_BUILD_DIR)/$(C_TOOLCHAIN_PACKAGE_NAME)/debian
	cp $(PROJECT_DIR)/tools/toolchains/musl-gcc/0014-libgomp-futex-occlum.diff $(DEB_BUILD_DIR)/$(C_TOOLCHAIN_PACKAGE_NAME)/debian
	cp $(PROJECT_DIR)/tools/installer/rpm/toolchains/musl-gcc/occlum-gcc.sh $(DEB_BUILD_DIR)/$(C_TOOLCHAIN_PACKAGE_NAME)/debian
	echo " echo 'Please execute command \"source /etc/profile\" to validate envs immediately' " > $(DEB_BUILD_DIR)/$(C_TOOLCHAIN_PACKAGE_NAME)/debian/occlum-gcc.postinst
	sed -i -e 's/$$MUSL_VERSION/$(MUSL_VERSION)/g' $(DEB_BUILD_DIR)/$(C_TOOLCHAIN_PACKAGE_NAME)/debian/changelog

golang: deps_golang
	cd $(DEB_BUILD_DIR)/occlum-toolchains-golang ; \
		dpkg-buildpackage -us -uc
	cp -t $(TARGET_DIR) $(DEB_BUILD_DIR)/occlum-toolchains-golang*.deb

deps_golang:
	mkdir -p $(TARGET_DIR)
	mkdir -p $(DEB_BUILD_DIR)/occlum-toolchains-golang
	cp -r toolchains/golang $(DEB_BUILD_DIR)/occlum-toolchains-golang/debian
	cp -t $(DEB_BUILD_DIR)/occlum-toolchains-golang/debian $(GO_PATCH) $(PROJECT_DIR)/tools/installer/rpm/toolchains/golang/occlum-go.sh

clean-build:
	rm -rf $(DEB_BUILD_DIR)
clean: clean-build
	rm -rf $(TARGET_DIR)
