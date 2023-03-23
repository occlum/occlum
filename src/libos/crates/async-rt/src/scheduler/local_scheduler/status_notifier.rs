/// Notify the status changes of a local scheduler.
///
/// Without notifications, the user should assume the target
/// local scheduler is not idle or sleeping initially.
///
/// Note that the idle and sleeping status are exclusive to each other:
/// a idle scheduler is not supposed to be sleeping, and vice versa.
///
/// Notifications made be delivered frequently. So the implementation
/// of this trait should be fast.
pub trait StatusNotifier: Send + Sync + 'static {
    /// Notify the changes of the idle status of the local scheduler.
    fn notify_idle_status(&self, vcpu: u32, is_idle: bool);

    /// Notify the changes of the sleep status of the local scheduler.
    fn notify_sleep_status(&self, vcpu: u32, is_sleep: bool);

    /// Notify the changes of the heavy status of the local scheduler
    fn notify_heavy_status(&self, vcpu: u32, is_heavy: bool);
}
