# OCCLUM EDMM 配置介绍

## EDMM环境要求
1. 硬件平台：需要支持SGX-2
2. OS: 需要Linux Kernel v6.0及以上版本，目前的开发测试主要使用的是v6.2

## EDMM启用方法
EDMM功能默认不启用。如需开启，在Occlum build阶段通过设置`ENABLE_EDMM`启用：
```bash
ENABLE_EDMM=Y occlum build
occlum run /xxx/xxx
```

如果未启用，则默认按照非EDMM平台进行build以及配置。

## Occlum.json配置

### 配置设计原则
一份Occlum.json文件可以同时运行在EDMM以及非EDMM平台。并且在EDMM平台能够自动地最大限度地使用EDMM的功能。

### EDMM原理
在支持EDMM后，`Enclave.xml`对于包括TCS, heap, stack等内存相关配置都提供类似`Init`和`Max`的两个配置项（有的还有第三个配置项 Min）。它的意义就是可以使得Enclave在启动阶段能够加载比较少的内存，来提高Enclave应用的启动速度。而把其他内存的加载时间平摊在运行时。`Occlum.json`由Occlum内部的配置工具转换成`Enclave.xml`并作为Enclave的最终配置。为了充分利用EDMM能力，因此也补充了相关的配置项。

总体配置思路大概是：如果希望Occlum启动速度比较快，就配置较少的`Init`内存。而当内存不足时，应该增加`Max`内存大小。`Init`内存作为Occlum启动时加载的内存，不管有没有真正用到都会被加载。`Max`内存决定了应用需要的最大内存，即使不用也不会浪费，在运行时进行按需加载。

### Occlum.json EDMM相关配置示例
总体是在原有`Occlum.json`基础上增加了3个选填字段，包括`kernel_space_heap_max_size`, `user_space_max_size`, 以及`init_num_of_threads`。打开全部EDMM相关配置后的`Occlum.json`内存部分配置如下：
```json
{
  "resource_limits": {
    "kernel_space_stack_size": "1MB",       // (已有，必填)
    "kernel_space_heap_size": "4MB",        // (已有，必填)
    "kernel_space_heap_max_size": "40MB",   // !!! (新增，选填)
    "user_space_size": "1MB",               // (已有，必填)
    "user_space_max_size": "600MB",         // !!! (新增，选填)
    "init_num_of_threads": 2,               // !!! (新增，选填)
    "max_num_of_threads": 64                // (已有，必填)
  },
  "process": {
    "default_stack_size": "4MB",            // (已有，必填)
    "default_heap_size": "8MB",             // (已有，必填)
    "default_mmap_size": "100MB"            // (已有，必填，但不生效)
  },
}
```

**如果没有提供必填项，则occlum build会失败。**

### 详细解释

[![](https://img.shields.io/badge/蓝色：表示已有的配置-lightblue?style=for-the-badge)]()
[![](https://img.shields.io/badge/绿色：表示新增的配置-lightgreen?style=for-the-badge)]()
[![](https://img.shields.io/badge/灰色：表示缺省的配置（由Occlum配置工具硬编码）-lightgrey?style=for-the-badge)]()


#### Kernel Stack

[![](https://img.shields.io/badge/kernel__space__stack__size-lightblue?style=flat)]() **对应Occlum kernel space stack内存, 与原有配置方式保持一致**

   - 对应`Enclave.xml`文件的StackMaxSize, StackMinSize (大小相同)
   - 由于该stack只为Occlum kernel thread提供, 因此内存需求并不会很大。一般都在1~4M，并且绝大多数时候都不需要改动。因此不再提供其他配置项
   - 推荐配置方式：与之前配置一致，不做修改


#### Kernel Heap

[![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() **对应Occlum kernel space heap内存**

   - 如果只配置了 [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]()
      - 在EDMM平台，Occlum配置工具会提供缺省的 [![](https://img.shields.io/badge/kernel__space__heap__max__size(目前为1GB)-lightgrey?style=flat)]() 作为按需分配的heap最大值，而 [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() 作为LibOS初始化时的kernel heap大小
      - 在非EDMM平台，[![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() 和之前的用法保持一致，对应静态的kernel space heap大小
 
   - 如果增加了 [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() 配置
      - 在EDMM平台，由 [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() 对比Occlum配置工具提供缺省的 [![](https://img.shields.io/badge/kernel__space__heap__max__size(目前为1GB)-lightgrey?style=flat)]() 的最大值作为kernel heap的最大值，而[![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]()作为LibOS初始化时的kernel heap大小
      - 在非EDMM平台，使用 [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() 作为静态的kernel space heap大小，[![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() **不再生效**

   - 推荐配置方式
      - EDMM平台：增加 [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() 配置，可以相比之前的 [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() 增加一些。原 [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() 可以根据期望的启动时间，适当减小。理论上 [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() 配置的越小，LibOS启动速度越快
      - 非EDMM平台：与之前配置一样，不做修改，即保持 [![](https://img.shields.io/badge/kernel__space__heap__size-lightblue?style=flat)]() 与之前一致。或者增加 [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() 配置来提供未来的拓展性


#### User Space

[![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() **对应Occlum user space 大小**

   - 如果只配置了 [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]()
      - 在EDMM平台，Occlum配置工具会提供缺省的 [![](https://img.shields.io/badge/user__space__max__size_(目前为16GB)-lightgray?style=flat)]() 作为按需分配的user space最大值，而[![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() 作为LibOS初始化时的user space大小
      - 在非EDMM平台，[![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() 和之前的用法保持一致，对应静态的user space大小
  
   - 如果增加了[![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() 配置
      - 在EDMM平台，由[![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]()对比Occlum配置工具提供缺省的 [![](https://img.shields.io/badge/user__space__max__size_(目前为16GB)-lightgray?style=flat)]() 的最大值作为user space的最大值，而 [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() 作为LibOS初始化时的user space大小
      - 在非EDMM平台，使用 [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() 作为静态的user space大小，[![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() **不再生效**

   - 推荐配置方式
      - EDMM平台：user space的内存变得廉价，所以 [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() 可以配置的比较大，比如在之前的 [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() 上乘以2倍，以杜绝应用OOM。原 [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() 可以根据期望的启动时间，适当减小。理论上 [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() 配置的越小，LibOS启动速度越快
      - 非EDMM平台：与之前配置一样，不做修改，即保持 [![](https://img.shields.io/badge/user__space__size-lightblue?style=flat)]() 与之前一致。也可以考虑增加 [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() 配置来提供未来的拓展性

#### TCS Number

[![](https://img.shields.io/badge/init__num__of__threads-lightgreen?style=flat)]() [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() **对应LibOS kernel线程和user线程数量的总和**

   - 和kernel heap以及user space大小配置不同的是，对于线程数量的配置是增加了对`Init`数量的配置，而保留了原有的`Max`数量的配置

   - 如果只配置了 [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]()
      - 在EDMM平台，Occlum配置工具会提供缺省的 [![](https://img.shields.io/badge/tcs__init__num_(目前为16)-lightgray?style=flat)]() 作为`Init`数量的线程 ，而由 [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() 对比Occlum配置工具提供缺省的 [![](https://img.shields.io/badge/tcs__max__num_(目前为4096)-lightgray?style=flat)]() 的最大值作为线程数量的最大值
      - 在非EDMM平台，[![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() 和之前的用法保持一致，对应静态的线程数量

   - 如果增加了 [![](https://img.shields.io/badge/init__num__of__threads-lightgreen?style=flat)]() 配置
      - 在EDMM平台，由 [![](https://img.shields.io/badge/init__num__of__threads-lightgreen?style=flat)]() 对比Occlum配置工具提供的缺省 [![](https://img.shields.io/badge/tcs__init__num_(目前为16)-lightgray?style=flat)]() 的最小值作为初始化的线程数量，而 [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() 对比Occlum配置工具提供缺省的 [![](https://img.shields.io/badge/tcs__max__num_(目前为4096)-lightgray?style=flat)]() 的最大值作为线程数量的最大值
      - 在非EDMM平台，[![](https://img.shields.io/badge/init__num__of__threads-lightgreen?style=flat)]() 不生效，只有 [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() 生效，与之前用法保持一致，对应静态的线程数量

   - 推荐配置方式
      - EDMM平台：增加 [![](https://img.shields.io/badge/init__num__of__threads-lightgreen?style=flat)]() 配置，可以根据期望的启动时间设定一个较小的值。理论上 [![](https://img.shields.io/badge/init__num__of__threads-lightgreen?style=flat)]() 配置的越小，LibOS的启动速度越快。[![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() 可以保持和之前的配置一样
      - 非EDMM平台：与之前配置一样，不做修改，即保持 [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]() 与之前一致


## 需要注意
1. 运行在EDMM平台会自动地最大限度使用EDMM功能。在目前的实现中，即使用户手动配置了 [![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() 或者 [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() , 也可能会超过这个限制而使用系统提供的 [![](https://img.shields.io/badge/缺省的max__size-lightgray?style=flat)]()。因此无法通过限制`max_size`来限制应用的最大物理内存用量。由于是按需分配的，这些内存如果没有用到的话，是不会占用物理内存的。

2. 如果需要考虑兼容性，即一份`Occlum.json`需要同时运行在EDMM和非EDMM平台，则请勿将`Max`相关值（包括[![](https://img.shields.io/badge/kernel__space__heap__max__size-lightgreen?style=flat)]() [![](https://img.shields.io/badge/user__space__max__size-lightgreen?style=flat)]() [![](https://img.shields.io/badge/max__num__of__threads-lightblue?style=flat)]()）设置的过大。在EDMM平台，由于内存按需提交，过大的`Max`值不影响程序正常运行，也不会造成浪费。但是在非EDMM平台，由于内存全量提交，`Max`值配置的比较大会导致LibOS启动速度慢，如果过大甚至会导致加载Enclave失败。

3. 环境变量`SGX_MODE`优先级高于`ENABLE_EDMM`。即在Simulation模式下无法启用EDMM。 如果同时配置了`ENABLE_EDMM=Y SGX_MODE=SIM`则只有`SGX_MODE`环境变量生效。

4. 用户应尽力避免`occlum build`时对EDMM的配置与`occlum run`时实际的EDMM配置冲突。如果`occlum build`和`occlum run`在不同环境：
   - `occlum build`时不启用EDMM, `occlum run`时环境支持EDMM: Occlum不会启用EDMM能力，依然按照非EDMM配置运行
   - `occlum build`时启用EDMM, `occlum run`时环境不支持EDMM: Enclave可以启动，但是由于配置文件的内存用量是按照EDMM环境进行配置的，在非EDMM环境会出现未定义错误。包括可能由于内存不足报错退出或发生panic等
