# Occlum Bom

## Introduction
This crate defines a file type `bom` to describe files that need to be copied into occlum image. The `bom` file is actually a `toml` file. User can generate a bom file with our tool, or write a `bom` file manually following the format of `toml`. `bom` file has three types of entries: 
- `include` means other bom files to be included. There's only one `include` entry in the array format in the file. A sample entry is 
```toml
include = [xxx.bom, yyy.bom]
```
- `files` means files to be copied into occlum image. There can be multiple file entries. A sample entry is
```toml
[[files]]
path = "xxxxxxx"
output_path = "yyyyyyyyy"
```
There's also optional field for `files`, `hash` and `target_executable`. If the field `hash` is set, we will check whether the hash value of the file content is consistent with the value in the bomfile when copy the file. If the field `target_executable` was set `true`, we will also copy the dependencies of this file when copy this file.
- `directories` means directories to be copied recursively into occlum image. A sample entry is
```toml
[[directories]]
path = "xxxxxxxxxxxxx"
output_path = "yyyyyyyyyyyyyy"
```

All entries support relative path or absolute path. If given relative path, it means relative path from the directory where we put the bom file. 

Then this crate provides three binary tools to manage bom files.
1. generate_bom: generate a bom file with user-specified input files or directories
2. copy_bom: copy files described in a bom file into occlum image
3. update_bom: update bom file due to content

More options of these tools can be seen by `<tool-name> --help`.

## How to install this tool

`cd occlum-bom && make install`
<font color="#660000"> Note: before use these tool, `rsync` should be installed to copy files (`cp` command can't support symbolic link) </font><br />
`apt install rsync -y`

## Example For hello_c in occlum docker image (After install this tool):
`cd demos/hello_c && make`
`occlum new occlum_workspace && cd occlum_workspace`
`generate_bom -e ../hello_world -o hello.bom -d image/bin`
`copy_bom -f hello.bom`
`update_bom -f hello.bom` (If need to update the bom file)
