//! Windows: keep the daemon from inheriting our stdio pipes.
//!
//! `CreateProcess` passes every *inheritable* handle to the child, and the std handles a process
//! received from its parent are inheritable. Without this, a daemon started by
//! `$(tok daemon start)` or a test's `Command::output()` holds the caller's pipe open, and the
//! caller waits for EOF until the daemon exits. Clearing the flag on our own std handles before
//! spawning fixes that; we keep using them normally.

#![allow(unsafe_code)]

use windows_sys::Win32::Foundation::{
    HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, SetHandleInformation,
};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};

/// Marks this process's stdin, stdout and stderr as not inheritable.
pub fn stop_inheriting_stdio() {
    for which in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: GetStdHandle has no preconditions. SetHandleInformation only changes the
        // inherit flag of a handle we own for the life of the process; a null or invalid handle
        // (no console) is skipped, and a failure leaves the handle as it was.
        unsafe {
            let h = GetStdHandle(which);
            if !h.is_null() && h != INVALID_HANDLE_VALUE {
                SetHandleInformation(h, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}
