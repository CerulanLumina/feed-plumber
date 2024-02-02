use feed_plumber_plugin_rs::{feed_plumber_plugin, FeedPlumberSink, FeedPlumberSource};
use serde::Deserialize;

feed_plumber_plugin! {
    sources: "counter" => CounterSource;
    sinks: "console" => ConsoleSink;
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
