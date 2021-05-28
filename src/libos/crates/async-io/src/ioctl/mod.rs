//! An ioctl-style, extensible API for file types.
//!
//! # Motiviation
//!
//! Let's consider three classic versatile APIs for files in Unix-like
//! systems: `ioctl`, `fcntl`, and `getsockopt`/`setsockopt`.
//!
//! ```c
//! int fcntl(int fd, int cmd, ... /* arg */ );
//!
//! int ioctl(int fd, unsigned long request, ... /* arg */);
//!
//! int getsockopt(int sockfd, int level, int optname,
//!                void *restrict optval, socklen_t *restrict optlen);
//! int setsockopt(int sockfd, int level, int optname,
//!                const void *optval, socklen_t optlen);
//! ```
//!
//! The three APIs have two properties in common: _extensibility_ and
//! _type erasure_. By extensibility, we mean that it is quite easy to add
//! new sub-commands that are specific to a device or file. And type erasure
//! underscores the fact that since the input and output arguments of a future
//! sub-command cannot be known beforehand, the concrete types of these
//! arguments have to be "erased" from the interface using `...` or `void*`.
//!
//! So how do we support these three C APIs in our Rust types for files?
//!
//! Specifically, do we need to add three separate Rust APIs corresponding to
//! their C counterparts? And what is the best way to express type-erased
//! arguments in the type-safe language of Rust? And most importantly, how
//! can we add new kinds of sub-commands and handle all kinds of sub-commands
//! as painless as possible?
//!
//! Our solution is the `IoctlCmd` trait and its companion macros.
//!
//! # Usage
//!
//! Here is a simple program to demonstrate the usage of `IoctlCmd`.
//!
//! ```rust
//! use async_io::prelude::*;
//! use async_io::{impl_ioctl_cmd, match_ioctl_cmd_mut};
//! use async_io::ioctl::{IoctlCmd};
//!
//! /// A trait to abstract any file-like type.
//! ///
//! /// For our purpose, it suffices for the trait to have only one API,
//! /// which takes a mutable trait object of `IoctlCmd` as its argument.
//! /// Thanks to `IoctlCmd`'s capability of downcasting itself to the
//! /// concrete type that implements the trait, it becomes easy to accept
//! /// all kinds of commands without breaking changes to the API signature.
//! pub trait File {
//!     fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()>;
//! }
//!
//! // A typical command consists of an input and an output. For such commands,
//! // you can use the `impl_ioctl_cmd` to define them handily.
//! //
//! // Here, three structs are defined, implementing the `IoctlCmd` trait.
//! // Each of them represent a different command, having different
//! // input and output arguments.
//! //
//! // Note that while the trait is named `IoctlCmd`, it does not
//! // preclude using the trait to abstract `fcntl` or `getsocktopt`
//! // commands.
//! impl_ioctl_cmd! {
//!     pub struct CommonCmd<Input=(), Output=bool> {}
//! }
//!
//! impl_ioctl_cmd! {
//!     pub struct FooCmd<Input=i32, Output=()> {}
//! }
//!
//! impl_ioctl_cmd! {
//!     pub struct BarCmd<Input=i32, Output=String> {}
//! }
//!
//! // Of course. You can implement an ioctl command manually, without
//! // using the `impl_ioctl_cmd` macro.
//! #[derive(Debug)]
//! pub struct ComplexCmd { /* customized memory layout */ };
//! impl IoctlCmd for ComplexCmd {}
//!
//! pub struct FooFile;
//! impl File for FooFile {
//!     fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
//!         // Only handle the interesting commands. The trait object
//!         // is automatically downcasted to the concrete struct that
//!         // represents the command.
//!         match_ioctl_cmd_mut!(&mut *cmd, {
//!             cmd: CommonCmd => {
//!                 println!("Accepted CommonCmd: {:?}", cmd);
//!                 Ok(())
//!             },
//!             cmd: FooCmd => {
//!                 println!("FooCmd's input: {}", cmd.input());
//!                 Ok(())
//!             },
//!             _ => {
//!                 Err(errno!(EINVAL, "unknown command"))
//!             }
//!         })
//!     }
//! }
//!
//! pub struct BarFile;
//! impl File for BarFile {
//!     fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
//!         match_ioctl_cmd_mut!(&mut *cmd, {
//!             cmd: CommonCmd => {
//!                 println!("Accepted CommonCmd: {:?}", cmd);
//!                 Ok(())
//!             },
//!             cmd: BarCmd => {
//!                 cmd.set_output("Bar Result".to_string());
//!                 println!("BarCmd's output: {}", cmd.output().unwrap());
//!                 Ok(())
//!             },
//!             cmd: ComplexCmd => {
//!                 println!("Accepted ComplexCmd: {:?}", cmd);
//!                 Ok(())
//!             },
//!             _ => {
//!                 Err(errno!(EINVAL, "unknown command"))
//!             }
//!         })
//!     }
//! }
//! ```
use std::fmt::Debug;

use downcast_rs::{impl_downcast, Downcast};

/// A trait to unify all concrete types representing ioctl commands.
///
/// The most useful property of this trait is that it supports downcasting
/// to the concrete type that actually implements the trait with the
/// `downcast_ref` and `downcast_mut` methods.
///
/// ```rust
/// use async_io::ioctl::IoctlCmd;
///
/// #[derive(Debug)]
/// pub struct DummyCmd;
/// impl IoctlCmd for DummyCmd {}
///
/// let dummy : Box<dyn IoctlCmd> = Box::new(DummyCmd);
/// assert!(dummy.downcast_ref::<DummyCmd>().is_some());
/// ```
pub trait IoctlCmd: Downcast + Debug {}
impl_downcast!(IoctlCmd);

/// A convenient macro to define a struct for some type of ioctl command.
///
/// Typcially, an ioctl command consists of an input argument and an output
/// output. As such, the struct defined by this macro has two fields: the
/// input and the output.
///
/// The struct defined by this macro automatically implements the `IoctlCmd`
/// trait.
#[macro_export]
macro_rules! impl_ioctl_cmd {
    (
        $(#[$outer:meta])*
        pub struct $CmdName:ident <Input=$Input:ty, Output=$Output:ty> {}
    ) => {
        $(#[$outer])*
        pub struct $CmdName {
            input: $Input,
            output: Option<$Output>,
        }

        #[allow(dead_code)]
        impl $CmdName {
            pub fn new(input: $Input) -> Self {
                Self {
                    input,
                    output: None,
                }
            }

            pub fn input(&self) -> &$Input {
                &self.input
            }

            pub fn output(&self) -> Option<&$Output> {
                self.output.as_ref()
            }

            pub fn set_output(&mut self, output: $Output) {
                self.output = Some(output)
            }

            pub fn take_output(&mut self) -> Option<$Output> {
                self.output.take()
            }
        }

        impl $crate::ioctl::IoctlCmd for $CmdName {}

        impl std::fmt::Debug for $CmdName  {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($CmdName))
                    .field("input", self.input())
                    .field("output", &self.output())
                    .finish()
            }
        }
    }
}

#[macro_export]
macro_rules! match_ioctl_cmd_ref {
    (
        $cmd:expr,
        {
            $( $bind:ident : $ty:ty => $arm:expr ),*,
            _ => $default:expr
        }
    ) => {{
        let __cmd : &dyn IoctlCmd = $cmd;
        $(
            if __cmd.is::<$ty>() {
                let $bind = __cmd.downcast_ref::<$ty>().unwrap();
                $arm
            } else
        )*
        {
            $default
        }
    }}
}

#[macro_export]
macro_rules! match_ioctl_cmd_mut {
    (
        $cmd:expr,
        {
            $( $bind:ident : $ty:ty => $arm:expr ),*,
            _ => $default:expr
        }
    ) => {{
        let __cmd : &mut dyn IoctlCmd = $cmd;
        $(
            if __cmd.is::<$ty>() {
                let $bind = __cmd.downcast_mut::<$ty>().unwrap();
                $arm
            } else
        )*
        {
            $default
        }
    }}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct WinSize(/* rows: */ u32, /* cols: */ u32);

    impl_ioctl_cmd! {
        // = ioctl TIOCSWINSZ
        #[derive(Default)]
        pub struct SetWinSize<Input=WinSize, Output=()> {}
    }

    impl_ioctl_cmd! {
        // = ioctl TIOCGWINSZ
        #[derive(Default)]
        pub struct GetWinSize<Input=(), Output=WinSize> {}
    }

    #[test]
    fn getter_setter() {
        let cmd = SetWinSize::new(WinSize(0, 0));
        assert!(cmd.input() == &WinSize(0, 0));
        assert!(cmd.output() == None);

        let mut cmd = GetWinSize::new(());
        assert!(cmd.input() == &());
        assert!(cmd.output() == None);

        let win_size = WinSize(1, 2);
        cmd.set_output(win_size);
        assert!(cmd.output() == Some(&win_size));

        let taken_win_size = cmd.take_output();
        assert!(taken_win_size == Some(win_size));
        assert!(cmd.output() == None);
    }

    #[test]
    fn match_boxed_cmd_ref() {
        let cmd: Box<dyn IoctlCmd> = Box::new(GetWinSize::new(()));
        match_ioctl_cmd_ref!(&*cmd, {
            cmd : GetWinSize => {
                println!("as expected: {:?}", cmd);
            },
            cmd : SetWinSize => {
                panic!("impossible: {:?}", cmd);
            },
            _ => {
                panic!("impossible");
            }
        });
    }

    #[test]
    fn match_boxed_cmd_mut() {
        let win_size = WinSize(1, 2);
        let mut cmd: Box<dyn IoctlCmd> = Box::new(SetWinSize::new(win_size));
        match_ioctl_cmd_mut!(&mut *cmd, {
            cmd : GetWinSize => {
                panic!("impossible: {:?}", cmd);
            },
            cmd : SetWinSize => {
                println!("as expected: {:?}", cmd);
            },
            _ => {
                panic!("impossible");
            }
        });
    }
}
