use std::fmt;

/// Soft-fail error when a renderer backend (OpenGL / wgpu) cannot start.
/// Returned to the host instead of panicking.
#[derive(Debug)]
pub struct InitError {
    pub message: String,
}

impl InitError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InitError {}
