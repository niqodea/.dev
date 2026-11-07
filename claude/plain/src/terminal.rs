//! The terminal's settings, which the client changes while it runs.

use std::io;
use std::mem::MaybeUninit;

/// Keeps the terminal from echoing keys, from holding them for a whole line, and from turning
/// Ctrl-C into a signal, until it is dropped. The input line reads keys this way while it is
/// open, and between two input lines a key then waits for the next one instead of showing up
/// where Claude's text goes.
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
        quiet.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
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
