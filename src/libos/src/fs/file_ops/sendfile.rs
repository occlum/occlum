use super::*;

pub async fn do_sendfile(
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
    if !in_file.access_mode().readable() {
        return_errno!(EBADF, "in_file is not readable");
    }
    let in_file_handle = in_file
        .as_async_file_handle()
        .ok_or_else(|| errno!(EINVAL, "not an inode"))?;
    let out_file = current.file(out_fd)?;
    if !out_file.access_mode().writable() {
        return_errno!(EBADF, "out_file is not writable");
    }
    let mut buffer: [u8; 1024 * 11] = unsafe { MaybeUninit::uninit().assume_init() };

    let mut read_offset = match offset {
        Some(offset) => offset as usize,
        None => in_file_handle.offset().await,
    };

    let in_file_inode = in_file_handle.dentry().inode();

    // read from specified offset and write new offset back
    let mut bytes_sent = 0;
    let mut send_error = None;
    while bytes_sent < count {
        let len = min(buffer.len(), count - bytes_sent);

        match in_file_inode.read_at(read_offset, &mut buffer[..len]).await {
            Ok(read_len) if read_len > 0 => match out_file.write(&buffer[..read_len]).await {
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
        in_file_handle
            .seek(SeekFrom::Current(bytes_sent as i64))
            .await?;
    }

    if bytes_sent > 0 {
        Ok((bytes_sent, read_offset))
    } else {
        send_error.map_or_else(|| Ok((0, read_offset)), |e| Err(e))
    }
}
