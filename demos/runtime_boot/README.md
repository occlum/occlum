# Runtime boot pre-generated UnionFS image

Generally, every Occlum instance has to pass the `Occlum build` process.
In some scenarios, mount and boot a pre-generated UnionFS image without `Occlum build` is a good feature. This demo introduces a way to runtime boot a BASH demo.

## Flow

### First, build a [`BASH`](../bash) Occlum instance

The later step will use the image content to generate UnionFS image.

### Build and start a [`gen_rootfs`](./gen_rootfs) Occlum instance

This `gen_rootfs` mounts a empty UnionFS, copy the BASH Occlum image content to mount point, unmount the UnionFS. It generates an encrypted UnionFS image containing the BASH image content. The **key** used in this demo is `"c7-32-b3-ed-44-df-ec-7b-25-2d-9a-32-38-8d-58-61"`.

### Build customized [`init`](./init)

Occlum default init calls syscall (363) `MountRootFS` for Occlum instance by `Occlum build`.
This customized init calls syscall (364) `MountRuntimeRootFS` to mount encrypted UnionFS image to boot.

### Build a boot template Occlum instance

This template uses the customized init. The RootFS image is not important, which will be replaced during boot.

### Add `Occlum.json.unprotected` to boot template Occlum instance

The `Occlum.json.unprotected` is the same with `Occlum.json` except the `entry_points` and image layer `source` is updated to adapt to the encrypted BASH UnionFS image.

All above steps could be done with one [`script`](./build_content.sh).

After running the script, runtime boot BASH could be done as below even if the default RootFS image has no BASH function.
```
# cd boot_instance
# occlum run /bin/occlum_bash_test.sh
```



