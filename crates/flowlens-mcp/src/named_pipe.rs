//! Named pipe server for MCP (Windows)
//!
//! HTTPDebuggerPro uses named pipes for MCP communication.
//! This module provides a named pipe transport for the MCP server.

#[cfg(windows)]
pub struct NamedPipeServer {
    pipe_name: String,
}

#[cfg(windows)]
impl NamedPipeServer {
    pub fn new(pipe_name: &str) -> Self {
        Self {
            pipe_name: pipe_name.to_string(),
        }
    }

    pub fn pipe_name(&self) -> &str {
        &self.pipe_name
    }
}

#[cfg(not(windows))]
pub struct NamedPipeServer {
    pipe_name: String,
}

#[cfg(not(windows))]
impl NamedPipeServer {
    pub fn new(pipe_name: &str) -> Self {
        Self {
            pipe_name: pipe_name.to_string(),
        }
    }

    pub fn pipe_name(&self) -> &str {
        &self.pipe_name
    }
}