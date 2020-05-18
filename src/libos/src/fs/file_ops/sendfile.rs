use super::*;

pub fn do_sendfile(
    out_fd: FileDesc,
    in_fd: FileDesc,
    offset: Option<off_t>,
    count: usize,
) -> Result<(usize, usize)> {
    // (len, offset)
    debug!(
        "sendfile: out: {}, in: {}, offset: {:?}, count: {}",
        out_fd, in_fd, offset, count
    );

    let current = current!();
    let in_file = current.file(in_fd)?;
    let out_file = current.file(out_fd)?;
    let mut buffer: [u8; 1024 * 11] = unsafe { MaybeUninit::uninit().assume_init() };

    let mut read_offset = match offset {
        Some(offset) => offset,
        None => in_file.seek(SeekFrom::Current(0))?,
    } as usize;

    // read from specified offset and write new offset back
    let mut bytes_read = 0;
    while bytes_read < count {
        let len = min(buffer.len(), count - bytes_read);
        let read_len = in_file.read_at(read_offset, &mut buffer[..len])?;
        if read_len == 0 {
            break;
        }
        bytes_read += read_len;
        read_offset += read_len;
        let mut bytes_written = 0;
        while bytes_written < read_len {
            let write_len = out_file.write(&buffer[bytes_written..read_len])?;
            if write_len == 0 {
                return_errno!(EBADF, "sendfile write return 0");
            }
            bytes_written += write_len;
        }
    }

    if offset.is_none() {
        in_file.seek(SeekFrom::Current(bytes_read as i64))?;
    }
    Ok((bytes_read, read_offset))
}
