use super::*;

pub struct RangeLockBuilder {
    // Mandatory field
    type_: Option<RangeLockType>,
    range: Option<FileRange>,
    // Optional fields
    owner: Option<pid_t>,
    waiters: Option<WaiterQueue>,
}

impl RangeLockBuilder {
    pub fn new() -> Self {
        Self {
            owner: None,
            type_: None,
            range: None,
            waiters: None,
        }
    }

    pub fn owner(mut self, owner: pid_t) -> Self {
        self.owner = Some(owner);
        self
    }

    pub fn type_(mut self, type_: RangeLockType) -> Self {
        self.type_ = Some(type_);
        self
    }

    pub fn range(mut self, range: FileRange) -> Self {
        self.range = Some(range);
        self
    }

    pub fn waiters(mut self, waiters: WaiterQueue) -> Self {
        self.waiters = Some(waiters);
        self
    }

    pub fn build(self) -> Result<RangeLock> {
        let owner = self.owner.unwrap_or_else(|| current!().process().pid());
        let type_ = self
            .type_
            .ok_or_else(|| errno!(EINVAL, "type_ is mandatory"))?;
        let range = self
            .range
            .ok_or_else(|| errno!(EINVAL, "range is mandatory"))?;
        let waiters = self.waiters;
        Ok(RangeLock {
            owner,
            type_,
            range,
            waiters,
        })
    }
}
