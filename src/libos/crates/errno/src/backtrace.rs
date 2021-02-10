use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use super::{Error, Result};

#[derive(Debug, Clone)]
pub struct ErrorBacktrace<'a> {
    next_error: Option<&'a Error>,
}

impl<'a> ErrorBacktrace<'a> {
    fn new(last_error: &'a Error) -> ErrorBacktrace {
        ErrorBacktrace {
            next_error: Some(last_error),
        }
    }
}

impl<'a> fmt::Display for ErrorBacktrace<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let error_strings: Vec<String> = self.clone().map(|e| alloc::format!("{}", e)).collect();
        let error_backtrace = error_strings.join("\n    Caused by ");
        write!(f, "{}", error_backtrace)
    }
}

impl<'a> Iterator for ErrorBacktrace<'a> {
    type Item = &'a Error;

    fn next(&mut self) -> Option<&'a Error> {
        if let Some(this_error) = self.next_error {
            self.next_error = this_error.get_cause().as_ref().map(|e| -> &Error { &e });
            return Some(this_error);
        }
        None
    }
}

impl Error {
    pub fn cause_err<F>(self, f: F) -> Error
    where
        F: FnOnce(&Error) -> Error,
    {
        let old_err = self;
        let mut new_err = f(&old_err);
        *new_err.get_cause_mut() = Some(Box::new(old_err));
        new_err
    }

    pub fn backtrace(&self) -> ErrorBacktrace {
        ErrorBacktrace::new(self)
    }
}

pub trait ResultExt<T> {
    fn cause_err<F>(self, f: F) -> Result<T>
    where
        F: FnOnce(&Error) -> Error;
}

impl<T> ResultExt<T> for Result<T> {
    fn cause_err<F>(self, f: F) -> Result<T>
    where
        F: FnOnce(&Error) -> Error,
    {
        self.map_err(|old_e| old_e.cause_err(f))
    }
}
