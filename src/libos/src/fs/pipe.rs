use super::*;

use async_io::ioctl::IoctlCmd;
pub use async_io::util::channel::{Channel, Consumer, Producer};

// TODO: Add F_SETPIPE_SZ in fcntl to dynamically change the size of pipe
// to improve memory efficiency. This value is got from /proc/sys/fs/pipe-max-size on linux.
pub const DEFAULT_BUF_SIZE: usize = 1024 * 1024;

pub fn do_pipe2(flags: u32) -> Result<[FileDesc; 2]> {
    let creation_flags = CreationFlags::from_bits_truncate(flags);
    let status_flags = StatusFlags::from_bits_truncate(flags);
    debug!("pipe2: flags: {:?} {:?}", creation_flags, status_flags);

    let (pipe_reader, pipe_writer) = pipe(status_flags)?;
    let pipe_writer = FileRef::new_file(pipe_writer);
    let pipe_reader = FileRef::new_file(pipe_reader);

    let close_on_spawn = creation_flags.must_close_on_spawn();
    let current = current!();
    let reader_fd = current.add_file(pipe_reader, close_on_spawn);
    let writer_fd = current.add_file(pipe_writer, close_on_spawn);
    debug!(
        "pipe2: returns reader_fd = {}, writer_fd = {}",
        reader_fd, writer_fd
    );
    Ok([reader_fd, writer_fd])
}

pub fn pipe(flags: StatusFlags) -> Result<(PipeReader, PipeWriter)> {
    let (producer, consumer) = Channel::with_capacity_and_flags(DEFAULT_BUF_SIZE, flags)?.split();

    Ok((PipeReader { consumer }, PipeWriter { producer }))
}

#[derive(Debug)]
pub struct PipeReader {
    consumer: Consumer,
}

impl File for PipeReader {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        self.consumer.read(buf)
    }

    fn access_mode(&self) -> AccessMode {
        self.consumer.access_mode()
    }

    fn status_flags(&self) -> StatusFlags {
        self.consumer.status_flags()
    }

    fn set_status_flags(&self, mut new_status_flags: StatusFlags) -> Result<()> {
        // Only O_NONBLOCK, O_ASYNC and O_DIRECT can be set
        new_status_flags &=
            (StatusFlags::O_NONBLOCK | StatusFlags::O_ASYNC | StatusFlags::O_DIRECT);

        self.consumer.set_status_flags(new_status_flags)
    }

    fn poll(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        self.consumer.poll(mask, poller)
    }

    fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) -> Result<()> {
        self.consumer.register_observer(observer, mask)
    }

    fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        self.consumer.unregister_observer(observer)
    }

    fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        async_io::match_ioctl_cmd_auto_error!(cmd, {
            cmd : GetReadBufLen => {
                let read_buf_len = self.consumer.ready_len();
                cmd.set_output(read_buf_len as _);
            },
        });
        Ok(())
    }
}

#[derive(Debug)]
pub struct PipeWriter {
    producer: Producer,
}

impl File for PipeWriter {
    fn write(&self, buf: &[u8]) -> Result<usize> {
        self.producer.write(buf)
    }

    fn access_mode(&self) -> AccessMode {
        self.producer.access_mode()
    }

    fn status_flags(&self) -> StatusFlags {
        self.producer.status_flags()
    }

    fn set_status_flags(&self, mut new_status_flags: StatusFlags) -> Result<()> {
        // Only O_NONBLOCK, O_ASYNC and O_DIRECT can be set
        new_status_flags &=
            (StatusFlags::O_NONBLOCK | StatusFlags::O_ASYNC | StatusFlags::O_DIRECT);

        self.producer.set_status_flags(new_status_flags)
    }

    fn poll(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        self.producer.poll(mask, poller)
    }

    fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) -> Result<()> {
        self.producer.register_observer(observer, mask)
    }

    fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        self.producer.unregister_observer(observer)
    }

    fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        async_io::match_ioctl_cmd_auto_error!(cmd, {});
        Ok(())
    }
}
