// Author Dustin Pilgrim
// License: MIT

use std::io::{self, Write};

#[derive(Debug, Clone, Copy)]
pub struct Log {
    pub quiet: bool,
    pub verbose: bool,
}

impl Log {
    pub fn info(&self, msg: impl AsRef<str>) {
        if self.quiet {
            return;
        }
        println!("{}", msg.as_ref());
    }

    pub fn warn(&self, msg: impl AsRef<str>) {
        if self.quiet {
            return;
        }
        let _ = writeln!(io::stderr(), "warning: {}", msg.as_ref());
    }

    pub fn error(&self, msg: impl AsRef<str>) {
        let _ = writeln!(io::stderr(), "error: {}", msg.as_ref());
    }

    /// Verbose “command tracing”. Goes to stderr to avoid polluting stdout.
    pub fn exec(&self, msg: impl AsRef<str>) {
        if !self.verbose || self.quiet {
            return;
        }
        let _ = writeln!(io::stderr(), "exec: {}", msg.as_ref());
    }
}

