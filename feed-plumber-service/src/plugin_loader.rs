use std::path::Path;

use anyhow::Context;
use log::{debug, error, info, warn};
use tap::{Tap, TapFallible, TapOptional};

use sys_feed_plumber_plugin::{InitializationFunction, INITIALIZATION_FUNCTION_NAME};

use crate::sys::{Plugin, PluginProcessorInstance, PluginSinkInstance, PluginSourceInstance};

pub struct PluginManager {
    plugins: Vec<Plugin>,
}

impl PluginManager {
    pub fn load(
        plugin_path: impl AsRef<Path>,
        additional: Vec<impl AsRef<Path>>,
    ) -> anyhow::Result<Self> {
        info!("Loading plugins from {:?}", plugin_path.as_ref());
        let dir = std::fs::read_dir(plugin_path).context("Unable to read plugin directory.")?;
        let mut plugins = Vec::new();
        for item in dir
            .filter_map(|res| {
                res.tap_err(|e| warn!("Unable to read directory entry, skipping. Details: {}", e))
                    .ok()
            })
            .map(|entry| {
                (
                    entry.path().tap(|a| {
                        a.file_name().tap_some(|a| debug!("Scanning {:?}", a));
                    }),
                    entry,
                )
            })
            .filter_map(|(path, entry)| {
                entry
                    .file_type()
                    .tap_err(|e| {
                        let name = path.file_name().unwrap_or(path.as_os_str());
                        warn!(
                            "Could not read file type of path {}. Details: {e}",
                            name.to_str()
                                .map(ToOwned::to_owned)
                                .unwrap_or_else(|| format!("{:?}", name))
                        );
                    })
                    .ok()
                    .and_then(|a| {
                        let b = a.is_file() || a.is_symlink();
                        if !b {
                            warn!("{:?} is not a file, cannot load.", &path);
                        }
                        b.then_some(path)
                    })
            })
            .chain(additional.into_iter().map(|a| a.as_ref().to_path_buf()))
        {
            let item_str = item.file_name().unwrap().to_string_lossy();
            info!("Attempting to load plugin from {item_str}");
            let plugin = unsafe {
                let initializer = libloading::Library::new(&item)
                    .tap_err(|err| warn!("Unable to load library {item_str}. Error: {err}"))
                    .and_then(|library| {
                        let library = Box::leak(Box::new(library));
                        library.get::<InitializationFunction>(INITIALIZATION_FUNCTION_NAME.as_bytes())
                    })
                    .tap_err(|err| warn!("Unable to find plugin initializer function in {item_str}. Error: {err}"));
                if let Ok(initializer) = initializer {
                    let raw_plugin = initializer();
                    Some(Plugin::from_raw(raw_plugin))
                } else {
                    None
                }
            };
            plugins.extend(plugin);
        }
        Ok(Self { plugins })
    }

    pub fn instantiate_source(
        &self,
        r#type: &str,
        name: String,
        config: &str,
    ) -> Option<PluginSourceInstance> {
        let plugin = self
            .plugins
            .iter()
            .find(|plugin| plugin.supplies_source(r#type))?;
        plugin
            .instantiate_source(r#type, name, config)
            .tap_none(|| {
                error!(
                    "Plugin declared source \"{}\" was available but was not able to instantiate.",
                    r#type
                )
            })
    }

    pub fn instantiate_sink(
        &self,
        r#type: &str,
        name: String,
        config: &str,
    ) -> Option<PluginSinkInstance> {
        let plugin = self
            .plugins
            .iter()
            .find(|plugin| plugin.supplies_sink(r#type))?;
        plugin.instantiate_sink(r#type, name, config).tap_none(|| {
            error!(
                "Plugin declared sink \"{}\" was available but was not able to instantiate.",
                r#type
            )
        })
    }

    pub fn instantiate_processor(
        &self,
        r#type: &str,
        name: String,
        config: &str,
    ) -> Option<PluginProcessorInstance> {
        let plugin = self
            .plugins
            .iter()
            .find(|plugin| plugin.supplies_processor(r#type))?;
        plugin
            .instantiate_processor(r#type, name, config)
            .tap_none(|| {
                error!(
                "Plugin declared processor \"{}\" was available but was not able to instantiate.",
                r#type
            )
            })
    }

    pub fn source_available(&self, r#type: &str) -> bool {
        self.plugins.iter().any(|a| a.supplies_source(r#type))
    }

    pub fn sink_available(&self, r#type: &str) -> bool {
        self.plugins.iter().any(|a| a.supplies_sink(r#type))
    }

    pub fn processor_available(&self, r#type: &str) -> bool {
        self.plugins.iter().any(|a| a.supplies_processor(r#type))
    }
}
