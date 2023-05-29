use std::env;
use std::fs;
use std::path::Path;
use nix::mount::{mount, umount, MsFlags};
use fs_extra::dir::{copy, CopyOptions};

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("{:?}", args);
    fs::create_dir("/mount").unwrap();

    let fs_type = "sefs";
    let mount_path = Path::new("/mount");
    let source = Path::new("sefs");
    let flags = MsFlags::empty();
    let key = &args[1];
    let options = format!(
        "dir={},key={}",
        "./mnt_unionfs/lower",
        key
    );

    println!("{:#?} {:#?} {:#?} {:#?} {:#?}", source, mount_path, fs_type, flags, options.as_str());
    mount(
        Some(source),
        mount_path,
        Some(fs_type),
        flags,
        Some(options.as_str()),
    ).unwrap();

    println!("Copy rootfs content to /mount");
    let copy_options = CopyOptions::new();
    let paths = fs::read_dir("/host/rootfs").unwrap();

    for entry in paths {
        let path = entry.unwrap().path();
        println!("Name: {}", path.display());
        copy(path, "/mount", &copy_options).unwrap();
    }

    println!("List directories in {:#?}", mount_path);
    let paths = fs::read_dir("/mount").unwrap();

    for path in paths {
        println!("Name: {}", path.unwrap().path().display())
    }

    println!("Unmount {:#?}", mount_path);
    umount(mount_path).unwrap();

    println!("Do mount again");
    fs::create_dir("/mount2").unwrap();
    let mount_path = Path::new("/mount2");
    println!("{:#?} {:#?} {:#?} {:#?} {:#?}", source, mount_path, fs_type, flags, options.as_str());
    mount(
        Some(source),
        mount_path,
        Some(fs_type),
        flags,
        Some(options.as_str()),
    ).unwrap();

    println!("List directories in {:#?}", mount_path);
    let paths = fs::read_dir("/mount2").unwrap();

    for path in paths {
        println!("Name: {}", path.unwrap().path().display())
    }

    println!("Unmount {:#?}", mount_path);
    umount(mount_path).unwrap();
}
