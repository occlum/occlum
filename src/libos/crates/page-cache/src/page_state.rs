/// Page state.
///
/// Recommended page state transition rule:
///
/// (Current State)       => New State
///
/// (None)                => Uninit,
///
/// (Fetching | Flushing) => UpToDate,
///
/// (Uninit | UpTodate)   => Dirty,
///
/// (Uninit)              => Fetching,
///
/// (Dirty)               => Flushing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageState {
    /// `Uninit` indicates a new allocated page which content has not been initialized.
    /// The page is available to write, not available to read.
    Uninit,
    /// `UpToDate` indicates a page which content is consistent with corresponding disk content.
    /// The page is available to read and write.
    UpToDate,
    /// `Dirty` indicates a page which content has been updated and not written back to underlying disk.
    /// The page is available to read and write.
    Dirty,
    /// `Fetching` indicates a page which content is being fetched from disk now.
    /// The page is not available to read or write.
    Fetching,
    /// `Flushing` indicates a page which content is being written back to underlying disk.
    /// The page is available to read, not available to write.
    Flushing,
}
