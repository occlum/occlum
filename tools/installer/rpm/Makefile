CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
PROJECT_DIR := $(CUR_DIR)/../../..
TARGET_DIR := $(abspath $(PROJECT_DIR)/build/rpms)

MAJOR_VER_NUM = $(shell grep '\#define OCCLUM_MAJOR_VERSION' $(PROJECT_DIR)/src/pal/include/occlum_version.h |  awk '{print $$3}')
MINOR_VER_NUM = $(shell grep '\#define OCCLUM_MINOR_VERSION' $(PROJECT_DIR)/src/pal/include/occlum_version.h |  awk '{print $$3}')
PATCH_VER_NUM = $(shell grep '\#define OCCLUM_PATCH_VERSION' $(PROJECT_DIR)/src/pal/include/occlum_version.h |  awk '{print $$3}')
VERSION_NUM := $(MAJOR_VER_NUM).$(MINOR_VER_NUM).$(PATCH_VER_NUM)

# Occlum specific version
MUSL_VERSION = 0.15.0
GOLANG_VERSION = 0.16.0

RPM_FILELIST = occlum-filelist occlum-pal-filelist occlum-runtime-filelist
EXPORT_SCRIPT = occlum-pal.sh occlum-runtime.sh

GCC_PATCH = toolchains/musl-gcc/musl-cross-make-disable-download.patch
GCC_SCRIPT = toolchains/musl-gcc/occlum-gcc.sh
OCCLUM_MUSL_PATCH := $(PROJECT_DIR)/tools/toolchains/musl-gcc/0014-libgomp-futex-occlum.diff

GO_PATCH = $(PROJECT_DIR)/tools/toolchains/golang/adapt-golang-to-occlum.patch
GO_SCRIPT = toolchains/golang/occlum-go.sh

PACKAGE_DIR = /tmp

.PHONY: all main main_deps musl-gcc deps_c clean golang

all: main

main: main_deps
	@mkdir -p $(TARGET_DIR)
	@rpmbuild -ba --define '_version $(VERSION_NUM)' --define '_musl_version $(MUSL_VERSION)' occlum.spec
	@mv $$HOME/rpmbuild $$HOME/rpmbuild-occlum
	@cp $$HOME/rpmbuild-occlum/RPMS/x86_64/*.rpm $(TARGET_DIR)
	@echo "$$(ls $$HOME/rpmbuild-occlum/RPMS/x86_64) can be found at $(TARGET_DIR)"

main_deps: $$HOME/rpmbuild/SOURCES/$(firstword $(EXPORT_SCRIPT)) $(PACKAGE_DIR)/$(VERSION_NUM).tar.gz

$$HOME/rpmbuild/SOURCES/$(firstword $(EXPORT_SCRIPT)):
	@if [ -d $$HOME/rpmbuild ]; then \
		mv $$HOME/rpmbuild $$HOME/rpmbuild-old-$$(date +%Y%m%d-%T); fi
	@mkdir -p $$HOME/rpmbuild/{BUILD,RPMS,SPECS,SOURCES,SRPMS}
	@cp -t $$HOME/rpmbuild/SOURCES $(RPM_FILELIST) $(EXPORT_SCRIPT)

$(PACKAGE_DIR)/$(VERSION_NUM).tar.gz:
	@cp -r $(PROJECT_DIR) $(PACKAGE_DIR)/occlum-$(VERSION_NUM)
	@cd $(PACKAGE_DIR)/occlum-$(VERSION_NUM); \
		git submodule deinit -f . ; \
		rm -rf $(PACKAGE_DIR)/occlum-$(VERSION_NUM)/build
	@cd $(PACKAGE_DIR) && tar -cvzf "$$HOME/rpmbuild/SOURCES/$(VERSION_NUM).tar.gz" occlum-$(VERSION_NUM)

musl-gcc: deps_c
	@rpmbuild -ba --define '_musl_version $(MUSL_VERSION)' toolchains/musl-gcc/occlum-toolchains-gcc.spec
	@mv $$HOME/rpmbuild $$HOME/rpmbuild-occlum-toolchains-gcc
	@cp $$HOME/rpmbuild-occlum-toolchains-gcc/RPMS/x86_64/*.rpm $(TARGET_DIR)
	@echo "$$(ls $$HOME/rpmbuild-occlum-toolchains-gcc/RPMS/x86_64) can be found at $(TARGET_DIR)"

deps_c: $$HOME/rpmbuild/SOURCES/$(GCC_PATCH)
$$HOME/rpmbuild/SOURCES/$(GCC_PATCH):
	@mkdir -p $(TARGET_DIR)
	@yum install -y rpmdevtools wget
	@if [ -d $$HOME/rpmbuild ]; then \
		mv $$HOME/rpmbuild $$HOME/rpmbuild-old-$$(date +%Y%m%d-%T); fi
	@mkdir -p $$HOME/rpmbuild/{BUILD,RPMS,SPECS,SOURCES,SRPMS}
	@MUSL_VERSION=$(MUSL_VERSION) toolchains/musl-gcc/download_sources.sh
	@spectool -g -R --define '_musl_version $(MUSL_VERSION)' toolchains/musl-gcc/occlum-toolchains-gcc.spec
	@cp -t $$HOME/rpmbuild/SOURCES $(GCC_PATCH) $(GCC_SCRIPT) $(OCCLUM_MUSL_PATCH)

golang: $$HOME/rpmbuild/SOURCES/$(GO_PATCH)
	@rpmbuild -ba --define '_golang_version $(GOLANG_VERSION)' toolchains/golang/occlum-toolchains-golang.spec
	@mv $$HOME/rpmbuild $$HOME/rpmbuild-occlum-toolchains-golang
	@cp $$HOME/rpmbuild-occlum-toolchains-golang/RPMS/x86_64/*.rpm $(TARGET_DIR)
	@echo "$$(ls $$HOME/rpmbuild-occlum-toolchains-golang/RPMS/x86_64) can be found at $(TARGET_DIR)"

$$HOME/rpmbuild/SOURCES/$(GO_PATCH):
	@mkdir -p $(TARGET_DIR)
	@yum install -y rpmdevtools
	@if [ -d $$HOME/rpmbuild ]; then \
		mv $$HOME/rpmbuild $$HOME/rpmbuild-old-$$(date +%Y%m%d-%T); fi
	@rm -rf $$HOME/rpmbuild/{BUILD,RPMS,SPECS,SRPMS} # tmp
	@mkdir -p $$HOME/rpmbuild/{BUILD,RPMS,SPECS,SOURCES,SRPMS}
	@spectool -g -R --define '_golang_version $(GOLANG_VERSION)' toolchains/golang/occlum-toolchains-golang.spec
	@cp -t $$HOME/rpmbuild/SOURCES $(GO_PATCH) $(GO_SCRIPT)

clean:
	@rm -rf $(PACKAGE_DIR)/occlum-$(VERSION_NUM)
	@rm -rf $$HOME/rpmbuild $$HOME/rpmbuild-occlum-toolchains* $$HOME/rpmbuild-occlum
	@rm -rf $(TARGET_DIR)
