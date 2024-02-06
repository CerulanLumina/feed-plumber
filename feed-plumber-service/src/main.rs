use std::{
    borrow::Cow,
    collections::HashMap,
    path::Path,
    sync::{Arc, Condvar, Mutex},
    thread,
    thread::sleep,
    time::Duration,
};

use crate::config::Pipeline;
use chrono::Local;
use clap::Parser;
use crossbeam::channel::Sender;
use log::{debug, error, warn, LevelFilter};
use tap::{TapFallible, TapOptional};

use crate::sys::{FeedPlumberComponentError, Items};

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

    let print_plugin_warnings = config.print_plugin_warnings;

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
            let sink_inst = plugin_manager
                .instantiate_sink(&sink.r#type, sink.name.clone(), &toml)
                .unwrap()
                .tap_err(|err| error!("Plugin processor \"{}\" could not be created due to an error. Plugin said: {err}", &sink.name));
            let Ok(mut sink_inst) = sink_inst else {
                return;
            };
            for item in recv {
                debug!("Sinking items to \"{}\"", sink_inst.name());
                sink_inst.sink_items(&item);
            }
        });
    }

    let mut processors_map = HashMap::new();
    for processor in config.processors {
        if !plugin_manager.processor_available(&processor.r#type) {
            error!(
                "Processor type \"{}\" unavailable. Skipping \"{}\"",
                &processor.r#type, &processor.name
            );
            continue;
        }
        let Ok(toml) = toml::to_string(&processor).tap_err(|err| debug!("{err}")) else {
            error!(
                "Unable to reserialize config for processor {}, skipping.",
                &processor.name
            );
            continue;
        };
        if processors_map.contains_key(&processor.name) {
            warn!("Duplicate processor name {}, skipping.", &processor.name);
            continue;
        }
        let (send, recv) = crossbeam::channel::unbounded::<ProcessorMessage>(); // TODO bounded
        processors_map.insert(processor.name.clone(), send);
        let plugin_manager = plugin_manager.clone();

        thread::spawn(move || {
            let processor_inst = plugin_manager
                .instantiate_processor(&processor.r#type, processor.name.clone(), &toml)
                .unwrap()
                .tap_err(|err| error!("Plugin processor \"{}\" could not be created due to an error. Plugin said: {err}", &processor.name));
            let Ok(mut processor_inst) = processor_inst else {
                return;
            };
            for ProcessorMessage {
                incoming,
                responder,
            } in recv
            {
                debug!("Processing items with \"{}\"", processor_inst.name());
                let res = processor_inst.process_items(&incoming);
                let items = match res {
                    Ok(items) => items,
                    Err(err) => {
                        match err {
                            FeedPlumberComponentError::Warn(err) => {
                                if print_plugin_warnings {
                                    warn!("Plugin processor \"{}\" errored while processing items: {err}", processor_inst.name());
                                }
                                Items::empty()
                            }
                            FeedPlumberComponentError::Fatal(err) => {
                                error!(
                                    "Plugin processor \"{}\" errored while processing items: {err}",
                                    processor_inst.name()
                                );
                                return;
                            }
                        }
                    }
                };
                if responder.send(items).is_err() {
                    error!(
                        "Processor \"{}\" responding to source that has hung up!",
                        processor_inst.name()
                    );
                }
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
        let mut senders = Vec::new();
        'pipeloop: for pipe in source.pipe {
            let mut proc_senders = Vec::new();
            for proc in &pipe.processors {
                if let Some(sender) = processors_map.get(proc) {
                    proc_senders.push(sender.clone());
                } else {
                    warn!(
                        "Pipeline invalid as processor {proc} does not exist. Pipeline: \"{pipe}\""
                    );
                    continue 'pipeloop;
                }
            }

            let pipe_senders = sinks_map
                .get(&pipe.sink)
                .cloned()
                .tap_none(|| {
                    warn!(
                        "Pipeline invalid as sink \"{}\" does not exist. Pipeline: \"{}\"",
                        &pipe.sink, pipe
                    )
                })
                .map(|a| ConstructedPipeline {
                    processors: proc_senders,
                    sink: a,
                    original: pipe,
                });

            senders.extend(pipe_senders);
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
            let mut errored = Vec::new();
            let source_inst = pm
                .instantiate_source(&source.r#type, source.name.clone(), &toml)
                .unwrap()
                .tap_err(|err| error!("Plugin source \"{}\" could not be created due to an error. Plugin said: {err}", &source.name));
            let Ok(mut source_inst) = source_inst else {
                return;
            };
            let mut upcoming = source.schedule.upcoming(Local);
            let mut next = upcoming.next().unwrap();
            loop {
                if next <= Local::now() {
                    next = upcoming.next().unwrap();
                    let source_items = source_inst.poll_source();
                    let source_items = match source_items {
                        Ok(source_items) => source_items,
                        Err(err) => match err {
                            FeedPlumberComponentError::Warn(err) => {
                                if print_plugin_warnings {
                                    warn!("Plugin source \"{}\" has errored while polling items. Skipping this batch. Plugin said {err}", &source.name);
                                }
                                Items::empty()
                            }
                            FeedPlumberComponentError::Fatal(err) => {
                                error!("Plugin source \"{}\" has errored while polling items. Disabling source. Plugin said {err}", &source.name);
                                return;
                            }
                        },
                    };
                    if !source_items.is_empty() {
                        'pipeline: for (idx, pipe) in senders.iter().enumerate() {
                            if errored.contains(&idx) {
                                continue;
                            }
                            let mut final_items = source_items.clone();
                            for (idx, proc) in pipe.processors.iter().enumerate() {
                                let (response_sender, response_receiver) =
                                    crossbeam::channel::bounded(0);
                                match proc.send(ProcessorMessage {
                                    incoming: final_items.clone(),
                                    responder: response_sender,
                                }) {
                                    Ok(_) => {
                                        final_items = response_receiver.recv().unwrap();
                                        if final_items.is_empty() {
                                            continue 'pipeline;
                                        }
                                    }
                                    Err(_) => {
                                        errored.push(idx);
                                        warn!("Pipeline \"{}\" on source \"{}\" has errored. Skipping for future polls.", &pipe.original, &source.name);
                                        continue 'pipeline;
                                    }
                                }
                            }
                            if final_items.is_empty() {
                                continue 'pipeline;
                            }
                            if pipe.sink.send(final_items).is_err() {
                                warn!("Pipeline \"{}\" on source \"{}\" has errored. Skipping for future polls.", &pipe.original, &source.name);
                                errored.push(idx);
                            }
                        }
                    } else {
                        debug!("Source \"{}\" returned no items.", &source.name);
                    }
                }
                sleep(Duration::from_millis(config.time_between_ticks as u64));
            }
        });
    }
    drop(condvar.wait(mutex_guard).unwrap());
    Ok(())
}

struct ConstructedPipeline {
    processors: Vec<Sender<ProcessorMessage>>,
    sink: Sender<Items>,
    original: Pipeline,
}

struct ProcessorMessage {
    incoming: Items,
    responder: Sender<Items>,
}
