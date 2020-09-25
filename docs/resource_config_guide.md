# Resource Configuration Guide

Due to SGX hardware limitations, Occlum needs users' assistance to specify some
resource limits or hints in `Occlum.json`. These configuration values must
be tuned on a per-application basis.

If you already know the memory usage of your application, set the memory usage
in the `process` field. Otherwise, use the default configuration file generated
after `occlum init`. During the tuning process, the log of Occlum should be
turned on by setting the `OCCLUM_LOG_LEVEL` environment variable (e.g.,
`OCCLUM_LOG_LEVEL=error`, `OCCLUM_LOG_LEVEL=info`).

To make the tuning process smooth, some troubleshooting instructions are listed
below. They can cover common types of errors resulting from the lack of
resources.

1. Mmap syscall error:
    - Error message: `[____Mmap] Error = ENOMEM (#12, Out of memory): not
      enough memory`

    - Solution: Enlarge `process.default_mmap_size`

2. Brk syscall error:
    - Error message: `[_____Brk] Error = EINVAL (#22, Invalid argument): New
      brk address is too high`

    - Solution: Enlarge `process.default_heap_size`

3. Process creation error:
    - Error message: `ENOMEM (#12, Out of memory): run out of reserved memory`

    - Solution: Enlarge `resource_limits.user_space_size`

4. Malloc of Rust SDK error:
    - Error message: `memory allocation of XXX bytes failed`

    - Solution: Enlarge `resource_limits.kernel_space_heap_size`

5. SGX protected file I/O error:
    - Error message: `SGX protected file I/O error: EIO (#5, I/O error): Cannot
      allocate memory (os error: 12)`

    - Solution: Enlarge `resource_limits.kernel_space_heap_size`

6. LibOS thread execution error:
    - Error message: `[ERROR] occlum-pal: Failed to enter the enclave to
      execute a LibOS thread (host tid = XXX): Unknown SGX error`

    - Solution: There are many reasons resulting in the above errors. Try to
      enlarge `resource_limits.max_num_of_threads` if your application has
      threads far more than it.
