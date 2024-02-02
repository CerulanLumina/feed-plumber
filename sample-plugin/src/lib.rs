use feed_plumber_plugin_rs::{
    feed_plumber_plugin, toml::Value, FeedPlumberProcessor, FeedPlumberSink, FeedPlumberSource,
};
use serde::Deserialize;
use std::collections::HashMap;

feed_plumber_plugin! {
    sources: "counter" => CounterSource;
    sinks: "console" => ConsoleSink;
    processors: "keymap" => KeyMapProcessor;
}

#[derive(Deserialize, Default)]
struct CounterSourceConfig {
    #[serde(default = "default_key")]
    key_name: String,
}

#[derive(Deserialize, Default)]
struct ConsoleSinkConfig {
    #[serde(default = "default_prefix")]
    prefix: String,
}

fn default_key() -> String {
    String::from("count")
}

fn default_prefix() -> String {
    "->".to_owned()
}

struct CounterSource {
    key: String,
    count: usize,
}

impl FeedPlumberSource for CounterSource {
    type ConfigType = CounterSourceConfig;

    fn new(config: Self::ConfigType) -> Self {
        CounterSource {
            key: config.key_name,
            count: 0,
        }
    }

    fn poll_source(&mut self) -> Vec<Vec<(String, String)>> {
        self.count += 1;
        vec![vec![(self.key.clone(), format!("{}", self.count))]]
    }
}

struct ConsoleSink {
    sequence: usize,
    prefix: String,
}

impl FeedPlumberSink for ConsoleSink {
    type ConfigType = ConsoleSinkConfig;

    fn new(config: Self::ConfigType) -> Self {
        Self {
            sequence: 1,
            prefix: config.prefix,
        }
    }

    fn sink_items(&mut self, items: Vec<Vec<(&str, &str)>>) {
        for item in items {
            println!("{:-<30}", "Item");
            for (key, value) in item {
                println!("\t{}{}{}: {}", self.sequence, self.prefix, key, value);
            }
            println!("{:-<30}", "-");
            self.sequence += 1;
        }
    }
}

struct KeyMapProcessor {
    from: String,
    to: String,
}

impl FeedPlumberProcessor for KeyMapProcessor {
    type ConfigType = HashMap<String, Value>;

    fn new(config: Self::ConfigType) -> Self {
        let from = config.get("from_key").unwrap().as_str().unwrap().to_owned();
        let to = config.get("to_key").unwrap().as_str().unwrap().to_owned();
        Self { from, to }
    }

    fn process_items(&mut self, items: Vec<Vec<(&str, &str)>>) -> Vec<Vec<(String, String)>> {
        items
            .into_iter()
            .map(|item| {
                item.into_iter()
                    .map(|(key, value)| {
                        let key = if key == self.from.as_str() {
                            self.to.clone()
                        } else {
                            key.to_owned()
                        };
                        (key, value.to_owned())
                    })
                    .collect()
            })
            .collect()
    }
}
