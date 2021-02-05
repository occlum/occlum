pub use io_uring_callback::IoUring;
/// A provider of an io_uring singleton.
///
/// This helper trait is intended to decouple the user of an IoUring instance
/// (here, a Socket instance) from the creater of an singleton of IoUring (e.g., a runtime).
/// And it frees each instance of a user type from storing their own reference to an io_uring
/// singleton---this info is embedded inside the type, not instances. This is a big win for
/// memory efficiency.
pub trait IoUringProvider: 'static + Send + Sync {
    type Instance: std::ops::Deref<Target = IoUring>;

    fn get_instance() -> Self::Instance;
}
