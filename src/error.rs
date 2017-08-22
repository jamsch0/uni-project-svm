use std::error;
use std::fmt;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    ProgramTooLarge,
    InvalidOpCode(u32),
    InvalidSysCall(u16)
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Call `description` with UFCS form to avoid collision/confusion
        // between trait `std::error::Error` and this crate's `Error`.
        write!(f, "Virtual Machine error: {}", error::Error::description(self))?;

        match *self {
            Error::InvalidOpCode(op) => write!(f, " (0b{:06b})", op),
            Error::InvalidSysCall(call) => write!(f, " (0x{:04x})", call),
            _ => Ok(())
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::ProgramTooLarge => "length of program exceeds 2^32 bytes",
            Error::InvalidOpCode(_) => "invalid opcode encountered",
            Error::InvalidSysCall(_) => "invalid syscall encountered"
        }
    }
}
