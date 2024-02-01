use std::{
    borrow::Cow,
    collections::HashMap,
    path::Path,
    sync::{Arc, Condvar, Mutex},
    thread,
    thread::sleep,
    time::Duration,
};

use chrono::Local;
use clap::Parser;
use log::{debug, error, warn, LevelFilter};
use tap::TapFallible;

use crate::sys::Items;

mod args;
mod config;
mod plugin_loader;
mod sys;

fn main() -> anyhow::Result<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(LevelFilter::Info)
        .parse_env("FEED_PLUMBER_LOG")
        .init();

    let condvar = Condvar::new();
    let mutex = Mutex::new(true);
    let mutex_guard = mutex.lock().unwrap();

    let opts = args::Opts::parse();

    let plugin_dir_path = opts
        .directory
        .map(Cow::from)
        .unwrap_or(Cow::Borrowed(Path::new("plugins")));
    let plugin_manager = Arc::new(
        plugin_loader::PluginManager::load(plugin_dir_path, opts.plugins)
            .tap_err(|err| error!("Unable to load plugins: {err}"))?,
    );

    let config_path = opts
        .config
        .map(Cow::from)
        .unwrap_or(Cow::Borrowed(Path::new("feedplumber.toml")));
    let config = config::load_from_toml(config_path)?;

    let mut sinks_map = HashMap::new();
    for sink in config.sinks {
        if !plugin_manager.sink_available(&sink.r#type) {
            error!(
                "Sink type \"{}\" unavailable. Skipping \"{}\"",
                &sink.r#type, &sink.name
            );
            continue;
        }
        let Ok(toml) = toml::to_string(&sink).tap_err(|err| debug!("{err}")) else {
            error!("Unable to reserialize config for {}, skipping.", &sink.name);
            continue;
        };
        if sinks_map.contains_key(&sink.name) {
            warn!("Duplicate sink name {}, skipping.", &sink.name);
            continue;
        }
        let (send, recv) = crossbeam::channel::unbounded::<Items>(); // TODO bounded
        sinks_map.insert(sink.name.clone(), send);
        let plugin_manager = plugin_manager.clone();

        thread::spawn(move || {
            let mut sink_inst = plugin_manager
                .instantiate_sink(&sink.r#type, sink.name, &toml)
                .unwrap();
            for item in recv {
                debug!("Sinking items to {}", sink_inst.name());
                sink_inst.sink_items(&item);
            }
        });
    }

    for source in config.sources {
        if !plugin_manager.source_available(&source.r#type) {
            error!(
                "Source type \"{}\" unavailable. Skipping \"{}\"",
                &source.r#type, &source.name
            );
            continue;
        }
        let Ok(toml) = toml::to_string(&source).tap_err(|err| debug!("{err}")) else {
            error!(
                "Unable to reserialize config for {}, skipping.",
                &source.name
            );
            continue;
        };
        // TODO: processors
        let mut senders = Vec::new();
        for pipe in &source.pipe {
            if let Some(sender) = sinks_map.get(&pipe.sink) {
                senders.push(sender.clone());
            } else {
                warn!(
                    "Pipeline invalid as sink \"{}\" does not exist. Pipeline: \"{}\"",
                    &pipe.sink, pipe
                );
            }
        }
        if senders.is_empty() {
            error!(
                "All pipelines for source \"{}\" are invalid. Skipping.",
                &source.name
            );
            continue;
        }
        let pm = plugin_manager.clone();
        thread::spawn(move || {
            let mut source_inst = pm
                .instantiate_source(&source.r#type, source.name, &toml)
                .unwrap();
            let mut upcoming = source.schedule.upcoming(Local);
            let mut next = upcoming.next().unwrap();
            loop {
                if next <= Local::now() {
                    next = upcoming.next().unwrap();
                    let items = source_inst.poll_source();
                    for send in &senders {
                        send.send(items.clone()).unwrap();
                    }
                }
                sleep(Duration::from_millis(config.time_between_ticks as u64));
            }
        });
    }
    drop(condvar.wait(mutex_guard).unwrap());
    Ok(())
}