/// Permission management module
/// 
/// This module handles process permissions, validation, and access control

pub mod definitions;
pub mod validator;

// Re-export commonly used types and functions
pub use definitions::*;
pub use validator::PermissionValidator; 