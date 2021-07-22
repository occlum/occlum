use super::*;

pub struct FlockBuilder {
    // Mandatory field
    owner: Option<ObjectId>,
    type_: Option<FlockType>,
    range: Option<FlockRange>,
    // Optional fields
    pid: Option<pid_t>,
    waiters: Option<WaiterQueue>,
    is_nonblocking: Option<bool>,
}

impl FlockBuilder {
    pub fn new() -> Self {
        Self {
            owner: None,
            type_: None,
            range: None,
            pid: None,
            waiters: None,
            is_nonblocking: None,
        }
    }

    pub fn owner(mut self, owner: ObjectId) -> Self {
        self.owner = Some(owner);
        self
    }

    pub fn type_(mut self, type_: FlockType) -> Self {
        self.type_ = Some(type_);
        self
    }

    pub fn range(mut self, range: FlockRange) -> Self {
        self.range = Some(range);
        self
    }

    pub fn pid(mut self, pid: pid_t) -> Self {
        self.pid = Some(pid);
        self
    }

    pub fn waiters(mut self, waiters: WaiterQueue) -> Self {
        self.waiters = Some(waiters);
        self
    }

    pub fn is_nonblocking(mut self, is_nonblocking: bool) -> Self {
        self.is_nonblocking = Some(is_nonblocking);
        self
    }

    pub fn build(self) -> Result<Flock> {
        let owner = self
            .owner
            .ok_or_else(|| errno!(EINVAL, "owner is mandatory"))?;
        let type_ = self
            .type_
            .ok_or_else(|| errno!(EINVAL, "type_ is mandatory"))?;
        let range = self
            .range
            .ok_or_else(|| errno!(EINVAL, "range is mandatory"))?;
        let pid = self.pid.unwrap_or_else(|| current!().process().pid());
        let waiters = self.waiters;
        let is_nonblocking = self.is_nonblocking.unwrap_or_else(|| true);
        Ok(Flock {
            owner,
            type_,
            range,
            pid,
            waiters,
            is_nonblocking,
        })
    }
}
