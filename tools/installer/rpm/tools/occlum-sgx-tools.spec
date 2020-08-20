%define centos_base_release 1

Name: occlum-sgx-tools
Version: 0.15.1
Release: %{centos_base_release}%{?dist}
Summary: Occlum sgx tools

Group: Development/Libraries
License: BSD License
URL: https://github.com/occlum/occlum
Source0: occlum-sgx-tools-filelist

ExclusiveArch: x86_64

%description
Occlum sgx tools used during `occlum build` and `occlum gdb`

%prep
mkdir -p %{?buildroot}
cp --parents /opt/intel/sgxsdk/lib64/{libsgx_ptrace.so,libsgx_uae_service_sim.so} %{?buildroot}
cp --parents /opt/intel/sgxsdk/lib64/gdb-sgx-plugin/* %{?buildroot}
cp --parents /opt/intel/sgxsdk/{bin/sgx-gdb,bin/x64/sgx_sign,environment,sdk_libs/libsgx_uae_service_sim.so} %{?buildroot}

%files
%files -f %{SOURCE0}

%changelog
* Wed Aug 05 2020 Chunyang Hui <sanqian.hcy@antfin.com> - 0.14.0-1
- Package init
