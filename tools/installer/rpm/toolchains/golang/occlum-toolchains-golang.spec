%define centos_base_release 1

%define INSTALL_DIR /opt/occlum/toolchains
%define GO_VERSION 1.13.7

# Skip no build id error
%undefine _missing_build_ids_terminate_build

# Skip stripping and building debug packages
%define __strip /bin/true
%define debug_package %{nil}
%define _unpackaged_files_terminate_build 0

Name: occlum-toolchains-golang
Version: %{_golang_version}
Release: %{centos_base_release}%{?dist}
Summary: occlum toolchains golang

Group: Development/Libraries
License: BSD License
URL: https://github.com/occlum/occlum
Source0: https://github.com/golang/go/archive/go%{GO_VERSION}.tar.gz
Source1: adapt-golang-to-occlum.patch
Source2: occlum-go.sh

ExclusiveArch: x86_64

BuildRequires: golang

%description
Occlum toolchains golang

%prep
%setup -q -c -n go-go%{GO_VERSION}
#%setup -q -c -T -D -a 1

# Apply the patch to adapt Golang to Occlum
cd go-go%{GO_VERSION}
patch -p1 < %{SOURCE1}

%build
cd go-go%{GO_VERSION}/src
# Disable compressed debug info
./make.bash

%install
mkdir -p %{buildroot}%{INSTALL_DIR}
mv go-go%{GO_VERSION} %{buildroot}%{INSTALL_DIR}/golang
rm -rf %{buildroot}%{INSTALL_DIR}/golang/.git*
cat > %{buildroot}%{INSTALL_DIR}/golang/bin/occlum-go <<EOF
#!/bin/bash
OCCLUM_GCC="\$(which occlum-gcc)"
OCCLUM_GOFLAGS="-buildmode=pie \$GOFLAGS"
CC=\$OCCLUM_GCC GOFLAGS=\$OCCLUM_GOFLAGS %{INSTALL_DIR}/golang/bin/go "\$@"
EOF
chmod +x %{buildroot}%{INSTALL_DIR}/golang/bin/occlum-go

# install occlum-go.sh
mkdir -p $RPM_BUILD_ROOT%{_sysconfdir}/profile.d
install -p -m 644 %{SOURCE2} $RPM_BUILD_ROOT%{_sysconfdir}/profile.d/

%files
/opt/occlum/toolchains/golang/*
/etc/profile.d/occlum-go.sh

%changelog
* Fri Sep 11 2020 Chunyang Hui <sanqian.hcy@antgroup.com> - 0.15.1-1
- package init
* Wed Jul 22 2020 Chunmei Xu <xuchunmei@linux.alibaba.com> - 0.14.0-1
- package init
