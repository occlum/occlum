//! Entrypoints subsystem of the LibOS.
//!
//! The entrypoints of the LibOS can be broadly classified into two categories:
//! * External entrypoints (i.e., ECalls) through which untrusted code can enter
//! into the enclave (see the `enclave` module).
//! * Internal entrypoints can be further classified into two categories:
//!     * User-to-LibOS entrypoints, which consists of three forms
//!         * Syscall (see the `syscall` module);
//!         * Exception (see the `exception` module);
//!         * Interrupt (see the `interrupt` module).
//!     * Thread entrypoint, where a LibOS thread starts execution (see the
//!     `thread` module).
//!
//! In addition to all sorts of entrypoints, the subsystem also includes modules
//! that facilitate the implementation of entrypoints, e.g., the `context_switch`
//! module.

pub mod context_switch;
pub mod enclave;
pub mod exception;
pub mod interrupt;
pub mod syscall;
pub mod thread;
