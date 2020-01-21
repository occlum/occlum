use super::*;

pub fn do_readlink(path: &str, buf: &mut [u8]) -> Result<usize> {
    info!("readlink: path: {:?}", path);
    let file_path = {
        if path == "/proc/self/exe" {
            let current_ref = process::get_current();
            let current = current_ref.lock().unwrap();
            current.get_elf_path().to_owned()
        } else if path.starts_with("/proc/self/fd") {
            let fd = path
                .trim_start_matches("/proc/self/fd/")
                .parse::<FileDesc>()
                .map_err(|e| errno!(EBADF, "Invalid file descriptor"))?;
            let current_ref = process::get_current();
            let current = current_ref.lock().unwrap();
            let file_ref = current.get_files().lock().unwrap().get(fd)?;
            if let Ok(inode_file) = file_ref.as_inode_file() {
                inode_file.get_abs_path().to_owned()
            } else {
                // TODO: support special device files
                return_errno!(EINVAL, "not a normal file link")
            }
        } else {
            // TODO: support symbolic links
            return_errno!(EINVAL, "not a symbolic link")
        }
    };
    let len = file_path.len().min(buf.len());
    buf[0..len].copy_from_slice(&file_path.as_bytes()[0..len]);
    Ok(len)
}
