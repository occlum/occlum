%define centos_base_release 1

%define GCC_VER 8.3.0
%define TARGET x86_64-linux-musl
%define INSTALL_DIR /opt/occlum/toolchains/gcc

# to skip no build id error
%undefine _missing_build_ids_terminate_build

Name: occlum-toolchains-gcc
Version: %{_musl_version}
Release: %{centos_base_release}%{?dist}
Summary: occlum toolchains gcc

Group: Development/Libraries
License: BSD License
URL: https://github.com/occlum/occlum
Source0: https://github.com/occlum/occlum/archive/%{_musl_version}.tar.gz
Source1: https://github.com/occlum/musl-cross-make/archive/0.9.9.hotfix.tar.gz
Source2: https://ftp.gnu.org/pub/gnu/gcc/gcc-%{GCC_VER}/gcc-%{GCC_VER}.tar.xz
Source3: config.sub
Source4: https://ftp.gnu.org/pub/gnu/binutils/binutils-2.33.1.tar.xz
Source5: https://ftp.gnu.org/pub/gnu/gmp/gmp-6.1.2.tar.bz2
Source6: https://ftp.gnu.org/pub/gnu/mpc/mpc-1.1.0.tar.gz
Source7: https://ftp.gnu.org/pub/gnu/mpfr/mpfr-4.0.2.tar.bz2
Source8: https://ftp.barfooze.de/pub/sabotage/tarballs/linux-headers-4.19.88.tar.xz
# Get Source9 from download script
Source9: musl-%{_musl_version}.tar.gz
Source10: occlum-gcc.sh

Patch0: musl-cross-make-disable-download.patch
Patch1: 0014-libgomp-futex-occlum.diff

ExclusiveArch: x86_64

BuildRequires: git

%description
Occlum toolchains gcc

%prep
%setup -q -c -n %{name}-%{version}
%setup -q -T -D -a 1

# This patch replaces syscall instruction with libc's syscall wrapper
cp %{PATCH1} musl-cross-make-0.9.9.hotfix/patches/gcc-%{GCC_VER}/

pushd musl-cross-make-0.9.9.hotfix
mkdir -p sources/gcc-%{GCC_VER}.tar.xz.tmp && cp %{SOURCE2} sources/gcc-%{GCC_VER}.tar.xz.tmp
mkdir -p sources/config.sub.tmp && cp %{SOURCE3} sources/config.sub.tmp
mkdir -p sources/binutils-2.33.1.tar.xz.tmp && cp %{SOURCE4} sources/binutils-2.33.1.tar.xz.tmp
mkdir -p sources/gmp-6.1.2.tar.bz2.tmp && cp %{SOURCE5} sources/gmp-6.1.2.tar.bz2.tmp
mkdir -p sources/mpc-1.1.0.tar.gz.tmp && cp %{SOURCE6} sources/mpc-1.1.0.tar.gz.tmp
mkdir -p sources/mpfr-4.0.2.tar.bz2.tmp && cp %{SOURCE7} sources/mpfr-4.0.2.tar.bz2.tmp
mkdir -p sources/linux-headers-4.19.88.tar.xz.tmp && cp %{SOURCE8} sources/linux-headers-4.19.88.tar.xz.tmp
tar xf %{SOURCE9}
%patch0 -p1
popd

%build
cd musl-cross-make-0.9.9.hotfix
cat > config.mak <<EOF
TARGET = %{TARGET}
COMMON_CONFIG += CFLAGS="-fPIC" CXXFLAGS="-fPIC" LDFLAGS="-pie"
GCC_VER = %{GCC_VER}
MUSL_VER = %{_musl_version}
EOF
make %{?_smp_mflags}

%install
mkdir -p %{buildroot}%{INSTALL_DIR}
cd musl-cross-make-0.9.9.hotfix
make install OUTPUT=%{buildroot}%{INSTALL_DIR}

# Generate the wrappers for executables
cat > %{buildroot}%{INSTALL_DIR}/bin/occlum-gcc <<EOF
#!/bin/bash
%{INSTALL_DIR}/bin/%{TARGET}-gcc -fPIC -pie -Wl,-rpath,%{INSTALL_DIR}/%{TARGET}/lib "\$@"
EOF

cat > %{buildroot}%{INSTALL_DIR}/bin/occlum-g++ <<EOF
#!/bin/bash
%{INSTALL_DIR}/bin/%{TARGET}-g++ -fPIC -pie -Wl,-rpath,%{INSTALL_DIR}/%{TARGET}/lib "\$@"
EOF

cat > %{buildroot}%{INSTALL_DIR}/bin/occlum-ld <<EOF
#!/bin/bash
%{INSTALL_DIR}/bin/%{TARGET}-ld -pie -rpath %{INSTALL_DIR}/%{TARGET}/lib "\$@"
EOF

chmod +x %{buildroot}%{INSTALL_DIR}/bin/occlum-gcc
chmod +x %{buildroot}%{INSTALL_DIR}/bin/occlum-g++
chmod +x %{buildroot}%{INSTALL_DIR}/bin/occlum-ld

mkdir -p %{buildroot}/lib
pushd %{buildroot}/lib
ln -sf %{INSTALL_DIR}/%{TARGET}/lib/libc.so ld-musl-x86_64.so.1
popd
mkdir -p %{buildroot}/usr/local
pushd %{buildroot}/usr/local
ln -sf %{INSTALL_DIR} occlum
popd
pushd %{buildroot}%{INSTALL_DIR}/bin
ln -sf %{INSTALL_DIR}/bin/x86_64-linux-musl-gcc-ar occlum-ar
ln -sf %{INSTALL_DIR}/bin/x86_64-linux-musl-strip occlum-strip
popd

# install occlum-gcc.sh
mkdir -p $RPM_BUILD_ROOT%{_sysconfdir}/profile.d
install -p -m 644 %{SOURCE10} $RPM_BUILD_ROOT%{_sysconfdir}/profile.d/

%files
/opt/occlum/toolchains/gcc/*
/usr/local/occlum
/lib/ld-musl-x86_64.so.1
/etc/profile.d/occlum-gcc.sh

%post
echo 'Please execute command "source /etc/profile" to validate envs immediately'

%changelog
* Wed Aug 05 2020 Chunyang Hui <sanqian.hcy@antfin.com> - 0.14.0-1
- Integrate with Occlum

* Mon Jul 20 2020 Chunmei Xu <xuchunmei@linux.alibaba.com> - 0.14.0-0
- Package init
