//! Simple logging utilities with compile-time verbosity control.
//!
//! Change VERBOSE to true/false before compiling to enable/disable debug logs.
//!
//! Use `log_debug!` for diagnostic messages that should only appear when verbose mode is on.

/// Set to `true` to enable verbose debug logging, `false` to suppress.
pub const VERBOSE: bool = true;

/// Log a debug message (only when VERBOSE is true).
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        if $crate::logging::VERBOSE && cfg!(not(test)) {
            hyperware_process_lib::println!($($arg)*);
        }
    };
}
