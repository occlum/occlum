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

    let in_file_access = in_file.access_mode()?;
    if !in_file_access.readable() {
        return_errno!(EBADF, "The in file is non-readable");
    }

    let out_file_access = out_file.access_mode()?;
    if !out_file_access.writable() {
        return_errno!(EBADF, "The out file is non-writable");
    }

    let mut buffer: [u8; 1024 * 11] = unsafe { MaybeUninit::uninit().assume_init() };

    let mut read_offset = match offset {
        Some(offset) => offset,
        None => in_file.seek(SeekFrom::Current(0))?,
    } as usize;

    // write_file is used to write buffer into out_file, the closure avoids complex loop structure
    let mut write_file = |buffer: &[u8]| -> Result<usize> {
        let buffer_len = buffer.len();
        let mut bytes_written = 0;
        let mut write_error = None;

        while bytes_written < buffer_len {
            match out_file.write(&buffer[bytes_written..]) {
                Ok(write_len) => {
                    debug_assert!(write_len > 0);
                    bytes_written += write_len;
                }
                Err(e) => {
                    // handle sendmsg return err
                    write_error = Some(e);
                    break;
                }
            }
        }

        if bytes_written > 0 {
            Ok(bytes_written)
        } else {
            // if bytes_written = 0, write_error must be Some(e).
            Err(write_error.unwrap())
        }
    };

    // read from specified offset and write new offset back
    let mut bytes_sent = 0;
    let mut send_error = None;
    while bytes_sent < count {
        let len = min(buffer.len(), count - bytes_sent);

        match in_file.read_at(read_offset, &mut buffer[..len]) {
            Ok(read_len) if read_len > 0 => match write_file(&buffer[..read_len]) {
                Ok(write_len) => {
                    bytes_sent += write_len;
                    read_offset += write_len;
                }
                Err(e) => {
                    send_error = Some(e);
                    break;
                }
            },
            Ok(..) => break,
            Err(e) => {
                send_error = Some(e);
                break;
            }
        }
    }

    if offset.is_none() {
        in_file.seek(SeekFrom::Current(bytes_sent as i64))?;
    }

    if bytes_sent > 0 {
        Ok((bytes_sent, read_offset))
    } else {
        send_error.map_or_else(|| Ok((0, read_offset)), |e| Err(e))
    }
}
