use super::*;

pub fn do_open(path: &str, flags: u32, mode: u32) -> Result<FileDesc> {
    info!(
        "open: path: {:?}, flags: {:#o}, mode: {:#o}",
        path, flags, mode
    );

    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();

    let file = proc.open_file(path, flags, mode)?;
    let file_ref: Arc<Box<dyn File>> = Arc::new(file);

    let fd = {
        let creation_flags = CreationFlags::from_bits_truncate(flags);
        proc.get_files()
            .lock()
            .unwrap()
            .put(file_ref, creation_flags.must_close_on_spawn())
    };
    Ok(fd)
}
