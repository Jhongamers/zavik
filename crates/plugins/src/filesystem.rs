//! Filesystem access plugin.

use zavik_shared::Error;
use zavik_sdk::{Capability, FileSystemAccess};
use std::path::Path;

/// Trivial [`FileSystemAccess`] implementation using `std::fs`.
#[derive(Debug, Default)]
pub struct LocalFileSystem;

impl Capability for LocalFileSystem {
    fn name(&self) -> &'static str {
        "file_system_access"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_file_system(&self) -> Option<&dyn FileSystemAccess> {
        Some(self)
    }
}

impl FileSystemAccess for LocalFileSystem {
    fn check_file_exists(&self, path: &Path) -> Result<bool, Error> {
        Ok(path.is_file())
    }

    fn read_to_string(&self, path: &Path) -> Result<String, Error> {
        std::fs::read_to_string(path)
            .map_err(|e| Error::IoError(format!("{}: {e}", path.display())))
    }

    fn write_string(&self, path: &Path, contents: &str) -> Result<(), Error> {
        std::fs::write(path, contents)
            .map_err(|e| Error::IoError(format!("{}: {e}", path.display())))
    }
}
