use super::*;

pub async fn do_sendfile(
    out_fd: FileDesc,
    in_fd: FileDesc,
    offset: Option<&mut off_t>,
    count: usize,
) -> Result<usize> {
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

    let mut bytes_sent = 0;
    let mut read_offset = match offset {
        Some(ref offset) => **offset as usize,
        None => in_file_handle.offset().await,
    };
    let try_send = async move || -> Result<()> {
        let mut buffer: [u8; 1024 * 11] = unsafe { MaybeUninit::uninit().assume_init() };
        // read from specified offset and write new offset back
        while bytes_sent < count {
            let len = min(buffer.len(), count - bytes_sent);
            let read_len = in_file_handle
                .dentry()
                .inode()
                .read_at(read_offset, &mut buffer[..len])
                .await?;
            if read_len == 0 {
                break;
            }
            let write_len = out_file.write(&buffer[..read_len]).await?;
            if write_len == 0 {
                break;
            }
            bytes_sent += write_len;
            read_offset += write_len;
        }
        Ok(())
    };

    let send_len = try_send().await.map_or_else(
        |e| {
            if bytes_sent > 0 {
                Ok(bytes_sent)
            } else {
                Err(e)
            }
        },
        |_| Ok(bytes_sent),
    )?;

    match offset {
        Some(offset) => {
            *offset += send_len as off_t;
        }
        None => {
            in_file_handle
                .seek(SeekFrom::Current(send_len as i64))
                .await?;
        }
    }

    Ok(send_len)
}
