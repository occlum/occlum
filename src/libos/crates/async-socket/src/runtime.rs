use io_uring_callback::IoUring;

/// The runtime support for HostSocket.
///
/// This trait provides a common interface for user-implemented runtimes
/// that support HostSocket. Currently, the only dependency is a singleton
/// of IoUring instance.
pub trait Runtime: Send + Sync + 'static {
    fn io_uring() -> &'static IoUring;
}
