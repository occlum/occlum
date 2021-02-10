use alloc::boxed::Box;
use core::fmt;

use super::{Errno, ToErrno};

#[derive(Debug)]
pub struct Error {
    inner: Error__,
    location: Option<ErrorLocation>,
    cause: Option<Box<Error>>,
}

#[derive(Debug)]
enum Error__ {
    Embedded((Errno, &'static str)),
    Boxed(Box<dyn ToErrno + 'static>),
}

#[derive(Debug, Clone, Copy)]
pub struct ErrorLocation {
    line: u32,
    file: &'static str,
}

impl Error {
    pub fn embedded(inner: (Errno, &'static str), location: Option<ErrorLocation>) -> Error {
        Error {
            inner: Error__::Embedded(inner),
            location: location,
            cause: None,
        }
    }

    pub fn boxed<T>(inner: T, location: Option<ErrorLocation>) -> Error
    where
        T: ToErrno + 'static,
    {
        Error {
            inner: Error__::Boxed(Box::new(inner)),
            location: location,
            cause: None,
        }
    }

    pub fn errno(&self) -> Errno {
        match &self.inner {
            Error__::Embedded((errno, _)) => *errno,
            Error__::Boxed(inner_error) => inner_error.errno(),
        }
    }

    pub fn get_cause_mut(&mut self) -> &mut Option<Box<Error>> {
        &mut self.cause
    }

    pub fn get_cause(&self) -> &Option<Box<Error>> {
        &self.cause
    }
}

impl ErrorLocation {
    pub fn new(file: &'static str, line: u32) -> ErrorLocation {
        ErrorLocation {
            file: file,
            line: line,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner)?;
        if let Some(location) = self.location {
            write!(f, " {}", location)?;
        }
        Ok(())
    }
}

impl fmt::Display for Error__ {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error__::Embedded((errno, msg)) => write!(f, "{}: {}", errno, msg),
            Error__::Boxed(inner_error) => write!(f, "{}: {}", inner_error.errno(), inner_error),
        }
    }
}

impl fmt::Display for ErrorLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[line = {}, file = {}]", self.line, self.file)
    }
}

#[cfg(any(feature = "std", feature = "sgx", test, doctest))]
mod if_std {
    use super::*;

    impl std::error::Error for Error {
        fn description(&self) -> &str {
            self.errno().as_str()
        }

        fn cause(&self) -> Option<&dyn std::error::Error> {
            self.cause.as_ref().map(|e| e as &dyn std::error::Error)
        }
        /*
           fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
               self.cause
                   .as_ref()
                   .map(|e| e as &(dyn std::error::Error + 'static))
           }
        */
    }
}
