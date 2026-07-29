//! App phases.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Boot,
    Running,
    Suspended,
    Shutdown,
}
