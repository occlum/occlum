use super::*;

#[derive(Clone, Copy, PartialEq)]
pub struct VMLayout {
    size: usize,
    align: usize,
}

impl VMLayout {
    pub fn new(size: usize, align: usize) -> Result<VMLayout> {
        if !align.is_power_of_two() || align % PAGE_SIZE != 0 {
            return_errno!(EINVAL, "invalid layout");
        }
        Ok(VMLayout { size, align })
    }

    pub fn new_empty() -> VMLayout {
        VMLayout {
            size: 0,
            align: PAGE_SIZE,
        }
    }

    pub fn extend(&mut self, more_space: &VMLayout) -> &mut Self {
        if more_space.size == 0 {
            return self;
        }

        self.size = align_up(self.size, more_space.align) + more_space.size;
        self.align = max(self.align, more_space.align);
        self
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn align(&self) -> usize {
        self.align
    }
}

impl fmt::Debug for VMLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VMLayout {{ size: 0x{:x?}, align: 0x{:x?} }}",
            self.size, self.align
        )
    }
}

impl Default for VMLayout {
    fn default() -> VMLayout {
        VMLayout::new_empty()
    }
}
