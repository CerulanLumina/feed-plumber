use feed_plumber_plugin_rs::{feed_plumber_plugin, FeedPlumberSink, FeedPlumberSource};

feed_plumber_plugin! {
    sources: "counter" => CounterSource;
    sinks: "console" => ConsoleSink;
}

struct CounterSource {
    count: usize,
}

impl FeedPlumberSource for CounterSource {
    fn new() -> Self {
        CounterSource { count: 0 }
    }

    fn poll_source(&mut self) -> Vec<Vec<(String, String)>> {
        self.count += 1;
        vec![vec![("count".to_owned(), format!("{}", self.count))]]
    }
}

struct ConsoleSink {
    prefix: String,
}

impl FeedPlumberSink for ConsoleSink {
    fn new() -> Self {
        Self {
            prefix: "".to_owned(),
        }
    }

    fn sink_items(&mut self, items: Vec<Vec<(&str, &str)>>) {
        for item in items {
            println!("{:-<30}", "Item");
            self.prefix.push('+');
            for (key, value) in item {
                println!("\t{}{}: {}", self.prefix, key, value);
            }
            println!("{:-<30}", "-");
        }
    }
}
