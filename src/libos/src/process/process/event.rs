use super::TermStatus;
use crate::events::Event;
use crate::signal::SigNum;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum StatusChange {
    Terminated(TermStatus),
    Stopped(SigNum),
    Continued(SigNum),
}

impl Event for StatusChange {}
