# Introduction
This crate provides a tool named `copy_bom`. This tool is designed to copy files described in a bom file to a given dest root directory.

# Bom file
Bom file is used to describe which files should be copied to the root directory(usually, the image directory). A bom file contains all files, directories that should be copied to the root directory. We also can define symbolic links and directories that should be created in a bom file. The meanings of each entry in the bom file can be seen in [RFC: bom file](https://github.com/occlum/occlum/issues/565). Also, a bom example with all optional entries can be seen in `example.yaml`.

# copy_bom
### overview
`copy_bom` is the tool designed to create directories and symbolic links, copy all files and directories defined in a bom file to the root directory. Internally, `copy_bom` will use `rsync` to do the real file operations. `copy_bom` will copy each file and directory incrementally, i.e., only changed parts will be copied. The permission bits and modification times will be reserved. This is done by the `-a` option of `rsync`. `copy_bom` will not ensure the whole image directory as described in bom file (sync behavior) because it will not try to delete old files. To pursue a sync behavior, one can delete the old image directory and copy files again.

### dependencies
`copy_bom` will analyze all dependencies(shared objects) of each ELF file via the dynamic loader defined in the `.interp` section in the file and automatically copy dependencies to the root directory. Currently, `copy_bom` only copy dependencies with absolute paths. We support only one dependency pattern in the result of dynamic loader.
- name => path   e.g., `libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6`  
All dependencies will be copied to the corresponding directory in root directory. For example, if root directory is `image`, then the dependency `/lib64/ld-linux-x86-64.so.2` will be copied to `image/lib/ld-linux-x86-64.so.2`. An entry named `autodep` with value `false` can be added to each file to avoid finding and copying dependencies automatically.

### log
`copy_bom` uses the same log setting as `occlum`. One can set `OCCLUM_LOG_LEVEL=trace` to see all logs printed by `copy_bom`.

### prepare and install
1.prepare. Since `copy_bom` relies on `rsync` to copy files. We need to install `rsync` at first. On ubuntu, this can be done by `apt install rsync -y`.
2.install. `copy_bom` can be installed via `cargo`. e.g., `cargo install --path .`.

### basic usage
`copy_bom [FLAGS] [OPTIONS] --file <bom-file> --root <root-dir>`
- bom-file: The bom file which describes files we want to copy.
- root-dir: The destination root directory we want to copy files to. Usually the `image` directory for occlum.

### options
- dry run mode: pass an option `--dry-run` to `copy_bom` will enable dry run mode. Dry run mode will output all file oprations in log but does not do real oprations. It is useul to check whether `copy_bom` performs as expected before doing real operations.

### flags
- -i, --include-dir: This flag is used to indicate which directory to find included bom files. This flag can be set multiple times. If the `include-dir` is set as a relative path, it is a path relative to the current path where you run the `copy_bom` command. 
- -h, --help: print help message

# known limitations

- The use of wildcard(like *) in files or directories is not supported. It may result in `copy_bom` behaving incorrectly.
- If we create symbolic link in bom file, it will always delete the old link and create a new one. It will change the modification time of the symbolic link.
- Environmental variables pointing to an empty value may fail to resolve.

# demos
1. The demos with `copy_bom` are in the `../../demos/bom-demos` directory.
2. Before using these demos, `rsync` and `copy_bom` should be installed. The file `base.yaml` should be copied to `/opt/occlum/etc/template`.
