use crate::reporter::ports::outbound::OutputPort;

pub struct TerminalOutput;

impl TerminalOutput {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TerminalOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputPort for TerminalOutput {
    fn write(&self, output: &str) {
        println!("{}", output);
    }
}
