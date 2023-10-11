# OCCLUM EDMM Configuration Guide

## EDMM Environment Requirements
1. Hardware Platform: SGX-2 support is required.
2. OS: Linux Kernel v6.0 or above is needed. And v6.2 is primarily used for development and testing.

## EDMM Enabling
EDMM feature is not enabled by default. To enable it, set `ENABLE_EDMM` during the Occlum build phase like below:
```bash
ENABLE_EDMM=Y occlum build
occlum run /xxx/xxx
```
If not enabled, Occlum will be built as running on the non-EDMM platform by default.

## Occlum.json Configuration

### Design Philosophy
A single Occlum.json file can be built for both EDMM and non-EDMM platforms while maximizing the EDMM functionality on the EDMM platform.

### EDMM Principle
To leverage EDMM, the `Enclave.xml` file offers two types of configuration options, namely `Init` and `Max`, for memory-related settings including TCS, heap, and stack (some may include a third option, `Min`). These options enable the Enclave to load a smaller memory footprint during the initialization phase, thereby enhancing the startup speed of the Enclave application. The remaining memory is loaded progressively at runtime in an on-demand manner. The `Enclave.xml` file is generated from the `Occlum.json` file using Occlum's internal configuration tool and serves as the definitive configuration file for the Enclave. To fully harness the capabilities of EDMM, additional configuration options are provided in the `Occlum.json` file.

The configuration strategy is as follows: to achieve a faster Occlum startup speed, configure a smaller value for the `Init` memory. In cases of insufficient memory availability, the size of the `Max` memory should be increased. The `Init` memory is loaded during Occlum's startup irrespective of its actual usage, while the `Max` memory determines the maximum memory requirement of the application and is dynamically loaded during runtime to avoid memory wastage.

### Occlum.json EDMM Configuration Introduction
In general, three optional fields have been added to the existing Occlum.json configuration: `kernel_space_heap_max_size`, `user_space_max_size`, and `init_num_of_threads`. An example of the memory section configuration in `Occlum.json` when all the EDMM-related configurations are enabled:

```json
{
  "resource_limits": {
    "kernel_space_stack_size": "1MB",       // (Legacy, Required)
    "kernel_space_heap_size": "4MB",        // (Legacy, Required)
    "kernel_space_heap_max_size": "40MB",   // !!! (Newly-Introduced，Optional)
    "user_space_size": "1MB",               // (Legacy, Required)
    "user_space_max_size": "600MB",         // !!! (Newly-Introduced，Optional)
    "init_num_of_threads": 2,               // !!! (Newly-Introduced，Optional)
    "max_num_of_threads": 64                // (Legacy, Required)
  },
  "process": {
    "default_stack_size": "4MB",            // (Legacy, Required)
    "default_heap_size": "8MB",             // (Legacy, Required)
    "default_mmap_size": "100MB"            // (Legacy, Required, but inoperative)
  },
}
```

**`occlum build` will fail if any of the `Required` field is not provided.**

### Detail Explanation

[![](https://img.shields.io/badge/Blue:_Legacy_Field-lightblue?style=for-the-badge)]()
[![](https://img.shields.io/badge/Green:_Newly--Introduced_Field-lightgreen?style=for-the-badge)]()
[![](https://img.shields.io/badge/Grey:_Default_Field_(Hardcoded_by_Occlum_internal_configuration_tool)-lightgrey?style=for-the-badge)]()


#### Kernel Stack

[![](https://img.shields.io/badge/kernel__space__stack__size-lightblue?style=flat)]() **corresponds to the Occlum kernel space stack memory and maintain consistency with the existing configuration method**

   - corresponds to StackMaxSize, StackMinSize of `Enclave.xml` file
   - Due to the fact that this stack is only intended for Occlum kernel threads, the memory requirement is not substantial. Typically, it ranges from 1 to 4MB, and in the majority of cases, no modifications are necessary. Consequently, no additional configuration options are provided
   - The recommended configuration approach is to keep it consistent with the previous settings without any modifications


#### Kernel Heap

[![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() **corresponds to the Occlum kernel space heap memory**

   - If only [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() is configured
      - On the EDMM platform, the default [![](https://img.shields.io/badge/kernel__space__heap__max__size(current_1GB)-lightgrey?style=flat)]() provided by the Occlum configuration tool serves as the maximum value for the heap allocated on-demand, while [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() represents the initial kernel heap size during LibOS initialization
      - On non-EDMM platforms, [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() remains consistent with its previous usage, corresponding to the static size of the kernel space heap

   - If [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() is newly added
      - On the EDMM platform, the maximum value provided by [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() is compared with the default [![](https://img.shields.io/badge/kernel__space__heap__max__size(current_1GB)-lightgrey?style=flat)]() from the Occlum configuration tool to determine the maximum value for the kernel heap. Meanwhile, [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() represents the initial kernel heap size during LibOS initialization
      - On non-EDMM platforms, [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() is used as the static size of the kernel space heap, and [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() **no longer takes effect**

   - Recommended configuration approach:
      - On the EDMM platform: Add [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() configuration field and can increase the  amount a little bit compared to the original [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)](). The [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() can be reduced appropriately based on the desired startup time. In theory, a smaller [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() configuration leads to faster LibOS startup speed.
      - On non-EDMM platforms: Keep the configuration consistent with the previous settings, i.e., no modifications to [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)](). Alternatively, adding [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() configuration field can provide future scalability.


#### User Space

[![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() **corresponds to Occlum's user space memory**

   - If only [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() is configured
      - On the EDMM platform, the default [![](https://img.shields.io/badge/user__space__max__size_(current_16GB)-lightgray?style=flat)]() provided by the Occlum configuration tool serves as the maximum value for the user space allocated on-demand, while [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() represents the initial user space size during LibOS initialization
      - On non-EDMM platforms, [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() remains consistent with its previous usage, corresponding to the static size of the user space

   - If [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() is newly added
      - On the EDMM platform, the maximum value provided by [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() is compared with the default [![](https://img.shields.io/badge/user__space__max__size_(current_16GB)-lightgray?style=flat)]() from the Occlum configuration tool to determine the maximum value for the user space. Meanwhile, [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() represents the initial user space size during LibOS initialization
      - On non-EDMM platforms, [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() is used as the static size of the user space, and [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() **no longer takes effect**

   - Recommended configuration approach:
      - On the EDMM platform: since we only pay for what we use, user space memory becomes more affordable. [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() can be configured to be relatively large, such as multiplying the previous value by `2`, to prevent the application OOM (out of memory) issues. The original [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() can be reduced appropriately based on the desired startup time. In theory, a smaller [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() configuration leads to faster LibOS startup speed
      - On non-EDMM platforms: Keep the configuration consistent with the previous settings, i.e., no modifications to [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)](). Alternatively, consider adding [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() configuration for future scalability


#### TCS Number

[![](https://img.shields.io/badge/init__num__of__threads-lightgreen?style=flat)]() [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() **corresponds to the total amount of threads used by LibOS kernel and user space**

   - Unlike the configuration of kernel heap and user space, the configuration for the TCS number introduces a separate configuration for the `Init` value, while retaining the original configuration for the `Max` number of threads

   - If only [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() is configured
      - On the EDMM platform, the Occlum configuration tool provides a default value of [![](https://img.shields.io/badge/tcs__init__num__(current_16)-lightgray?style=flat)]() for the number of `Init` threads, while [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() is compared with the default value of [![](https://img.shields.io/badge/tcs__max__num_(current_4096)-lightgray?style=flat)]() from the Occlum configuration tool to determine the maximum number of threads
      - On non-EDMM platforms, [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() remains consistent with its previous usage, corresponding to the static number of threads

   - If [![](https://img.shields.io/badge/init__num__of__threads-lightgreen?style=flat)]() is newly added
      - On the EDMM platform, the minimum value of [![](https://img.shields.io/badge/init__num__of__threads-lightgreen?style=flat)]() compared with the default value of [![](https://img.shields.io/badge/tcs__init__num_(current_16)-lightgray?style=flat)]() provided by the Occlum configuration tool is used as the number of `Init` threads during initialization, while [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() is compared with the default value of [![](https://img.shields.io/badge/tcs__max__num_(current_4096)-lightgray?style=flat)]() from the Occlum configuration tool to determine the maximum number of threads
      - On non-EDMM platforms, [![](https://img.shields.io/badge/init__num__of__threads-lightgreen?style=flat)]() does not take effect, and only [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() is effective, remaining consistent with its previous usage, corresponding to the static number of threads

   - Recommended configuration approach
      - On the EDMM platform: Add the [![](https://img.shields.io/badge/init__num__of__threads-lightgreen?style=flat)]() configuration field and set a smaller value based on the desired startup time. In theory, a smaller [![](https://img.shields.io/badge/init__num__of__threads-lightgreen?style=flat)]() configuration leads to faster LibOS startup speed. [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() can remain the same as the previous configuration
      - On non-EDMM platforms: Keep the configuration consistent with the previous settings, i.e., no modifications to [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() .


## Notes
1. When running Occlum on the EDMM platform, it will automatically make maximum use of the EDMM functionality. In the current implementation, even if the user manually configures [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() or [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)](), it may still exceed this limit and use the default system-provided [![](https://img.shields.io/badge/max__size-lightgray?style=flat)](). Therefore, limiting the `Max` setting cannot effectively restrict the maximum physical memory usage of the application. However, these physical memory will not be wasted if they are not used, since the memory is allocated on-demand.

2. If compatibility needs to be considered, i.e., an `Occlum.json` file needs to run on both EDMM and non-EDMM platforms, it is advisable not to set the max-related values (including [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() ) too large. On the EDMM platform, due to memory being committed on demand, setting a large `Max` value does not affect normal program execution or cause wastage. However, on non-EDMM platforms, where memory is fully committed, a large `Max` value may result in slower LibOS startup speed and could even cause Enclave loading failures.

3. The environment variable `SGX_MODE` takes precedence over `ENABLE_EDMM`. This means EDMM cannot be enabled in the SGX Simulation mode. If both `ENABLE_EDMM=Y` and `SGX_MODE=SIM` are configured, only the `SGX_MODE` environment variable takes effect.

4. Users should try to avoid conflicts between the EDMM configuration during `occlum build` and the actual execution environment during `occlum run`. If `occlum build` and `occlum run` are performed in different environments:
   - If EDMM is not enabled during `occlum build` but the environment supports EDMM during `occlum run`: Occlum will not enable EDMM capabilities and will run with the non-EDMM configuration
   - If EDMM is enabled during `occlum build` but the environment does not support EDMM during `occlum run`: The Enclave can still start, but since the memory usage of the configuration file is targeted for the EDMM environment, undefined errors may occur in the non-EDMM environment. This includes explicit errors or panics due to insufficient memory
