//! Crate `devforge-kernel` — the **Microkernel** (ADR 0001).
//!
//! Responsibilities (and nothing beyond them):
//! 1. Manage the plugin lifecycle (registration/discovery).
//! 2. Publish system events to the `EventBus` (ADR 0002).
//! 3. Perform the "safe cast" of capabilities when a use case needs
//!    them (`arquiteture.md`, section 3).
//!
//! The Kernel holds no business rules and knows nothing about UI/frameworks.

use devforge_events::{EventBus, EventEnvelope};
use devforge_shared::{Context, Error, ProjectStack};
use devforge_sdk::{
    Capability, CommandRunner, FileSystemAccess, PluginContext, ProjectAnalyzer, ProjectDetector,
};
use std::collections::HashMap;

/// Central orchestrator.
pub struct Kernel {
    plugins: HashMap<String, Box<dyn Capability>>,
    event_bus: EventBus,
}

impl Default for Kernel {
    fn default() -> Self {
        Self::new()
    }
}

impl Kernel {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            event_bus: EventBus::new(),
        }
    }

    /// Access to the bus (for the presentation layer to subscribe).
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    /// Registers a plugin (simulates `plugin.on_load(context)`).
    ///
    /// Silently replaces a plugin with the same name, which also allows
    /// mocks in tests.
    pub fn load_plugin(
        &mut self,
        plugin: Box<dyn Capability>,
        _context: &PluginContext,
    ) -> Result<(), String> {
        let name = plugin.name().to_string();
        eprintln!("KERNEL: registrando plugin '{name}'");
        self.plugins.insert(name, plugin);
        Ok(())
    }

    /// Starts the Kernel by publishing the first system event.
    pub fn run_initialization(&self) {
        eprintln!("KERNEL: DevForge inicializado.");
        self.event_bus.publish(EventEnvelope::new("system_ready"));
    }

    /// Stack detection capability, if some plugin provides it.
    pub fn detector(&self) -> Option<&dyn ProjectDetector> {
        self.plugins
            .get("stack_detector")
            .and_then(|plugin| plugin.as_detector())
    }

    /// File system access capability, if some plugin provides it.
    pub fn file_system(&self) -> Option<&dyn FileSystemAccess> {
        self.plugins
            .get("file_system_access")
            .and_then(|plugin| plugin.as_file_system())
    }

    /// Command execution capability, if some plugin provides it.
    pub fn command_runner(&self) -> Option<&dyn CommandRunner> {
        self.plugins
            .get("command_runner")
            .and_then(|plugin| plugin.as_command_runner())
    }

    /// Dependency analysis capability, if some plugin provides it.
    pub fn analyzer(&self) -> Option<&dyn ProjectAnalyzer> {
        self.plugins
            .get("dependency_analyzer")
            .and_then(|plugin| plugin.as_analyzer())
    }

    /// Minimal use case (POC): checks whether a `README.md` exists at the root.
    ///
    /// Uses the `FileSystemAccess` capability when available; otherwise it
    /// falls back to the file system directly (useful for isolated tests).
    pub fn check_readme_exists(&self, context: &Context) -> Result<bool, Error> {
        let path = context.project_root.join("README.md");
        if let Some(fs) = self.file_system() {
            return fs.check_file_exists(&path);
        }
        Ok(path.is_file())
    }

    /// Stack detection using the registered plugin (error if there is none).
    pub fn detect_stacks(&self, context: &Context) -> Result<Vec<ProjectStack>, Error> {
        match self.detector() {
            Some(detector) => detector.detect(context),
            None => Err(Error::Unknown(
                "nenhum plugin 'stack_detector' registrado no Kernel".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn finds_existing_readme() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        std::fs::write(dir.path().join("README.md"), "conteúdo")?;
        let context = Context {
            project_root: dir.path().to_path_buf(),
        };

        assert!(Kernel::new().check_readme_exists(&context)?);
        Ok(())
    }

    #[test]
    fn does_not_find_missing_readme() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let context = Context {
            project_root: dir.path().to_path_buf(),
        };

        assert!(!Kernel::new().check_readme_exists(&context)?);
        Ok(())
    }

    #[test]
    fn publishes_event_to_subscribers() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let kernel = Kernel::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        kernel.event_bus().subscribe(move |event| {
            if event.event_type == "system_ready" {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }
        });

        kernel.run_initialization();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
