use super::*;

impl super::VMArea {
    pub fn get_flags(&self) -> &VMAreaFlags {
        &self.flags
    }

    pub fn get_flags_mut(&mut self) -> &mut VMAreaFlags {
        &mut self.flags
    }
}
