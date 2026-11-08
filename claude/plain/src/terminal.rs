//! The terminal's settings, which the client changes while it runs.

use std::io;
use std::mem::MaybeUninit;

/// Hands the client every key as typed until it is dropped: the terminal echoes none, holds none
/// for a whole line, and takes none for itself, like Ctrl-C for a signal, Ctrl-S to pause the
/// output or Ctrl-V to quote the next key. Its output stays as it was, so a line break still
/// starts the next row at its start. Between two readings of the input line, a key then waits
/// for the next one instead of showing up where Claude's text goes.
pub struct QuietTerminal {
    original: libc::termios,
}

impl QuietTerminal {
    pub fn new() -> Result<Self, String> {
        let mut settings = MaybeUninit::<libc::termios>::uninit();
        // SAFETY: tcgetattr fills the termios when it returns 0
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, settings.as_mut_ptr()) } == -1 {
            let error = io::Error::last_os_error();
            return Err(format!("Cannot read the terminal's settings: {error}"));
        }
        // SAFETY: tcgetattr returned 0 above, so it filled the termios
        let original = unsafe { settings.assume_init() };
        let mut quiet = original;
        quiet.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG | libc::IEXTEN);
        quiet.c_iflag &= !(libc::IXON | libc::ICRNL);
        // SAFETY: the termios is a valid copy of the terminal's own settings
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw const quiet) } == -1 {
            let error = io::Error::last_os_error();
            return Err(format!("Cannot change the terminal's settings: {error}"));
        }
        Ok(QuietTerminal { original })
    }
}

impl Drop for QuietTerminal {
    fn drop(&mut self) {
        // SAFETY: the termios holds the settings the terminal had before.
        // Restoring them can only fail if the terminal is gone, which leaves
        // nothing to restore
        unsafe {
            libc::tcsetattr(
                libc::STDIN_FILENO,
                libc::TCSADRAIN,
                &raw const self.original,
            )
        };
    }
}
