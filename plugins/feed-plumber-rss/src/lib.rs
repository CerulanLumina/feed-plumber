use anyhow::{anyhow, Context};
use feed_plumber_plugin_rs::{
    feed_plumber_fatal, feed_plumber_plugin, toml::Value, FeedPlumberSource,
};
use reqwest::Url;
use std::{
    collections::{HashMap, HashSet},
    fs,
    fs::File,
    io::ErrorKind,
};

feed_plumber_plugin! {
    sources: "feed" => FeedSource;
}

struct FeedSource {
    feed_url: Url,
    max_size: usize,
}

impl FeedPlumberSource for FeedSource {
    type ConfigType = HashMap<String, Value>;

    fn new(config: Self::ConfigType) -> anyhow::Result<Self> {
        let feed_url = config
            .get("feed")
            .ok_or(anyhow!("No `feed` property"))
            .and_then(|a| a.as_str().ok_or(anyhow!("`feed` property not string")))
            .and_then(|a| Url::parse(a).context("`feed` property invalid URL"))?;
        let max_size = config
            .get("seen_file_max_size")
            .map(|val| {
                val.as_integer()
                    .ok_or(anyhow!("`seen_file_max_size` is not an integer"))
                    .and_then(|a| {
                        if a > 0 {
                            Ok(a as usize)
                        } else {
                            Err(anyhow!("`seen_file_max_size` cannot be negative: {a}"))
                        }
                    })
            })
            .unwrap_or(Ok(10_000_000))?;

        Ok(FeedSource { feed_url, max_size })
    }

    fn poll_source(&mut self) -> anyhow::Result<Vec<Vec<(String, String)>>> {
        let hashset: HashSet<String> = match fs::read("rss-seen.dat") {
            Ok(file) => {
                let res =
                    miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&file, self.max_size)
                        .map_err(|err| anyhow!("Decompressing: {err:?}"))
                        .and_then(|data| rmp_serde::from_slice(&data).context("Deserializing"));
                match res {
                    Ok(hash) => hash,
                    Err(err) => {
                        feed_plumber_fatal!("Unable to read rss-seen.dat: {err:?}");
                    }
                }
            }
            Err(err) => {
                if matches!(err.kind(), ErrorKind::NotFound) {
                    HashSet::new()
                } else {
                    feed_plumber_fatal!("Unable to read rss-seen.dat: {err:?}");
                }
            }
        };
        reqwest::blocking::get(self.feed_url.clone())
            .context("Requesting feed")
            .and_then(|res| res.error_for_status().context("Server returned HTTP error"))
            .and_then(|res| {
                feed_rs::parser::Builder::new()
                    .build()
                    .parse(res)
                    .context("Parsing feed")
            })
            .and_then(|feed| {
                let mut v = Vec::with_capacity(feed.entries.len());
                for entry in feed.entries {
                    let mut pairs = Vec::new();
                    if let Some(title) = entry.title { pairs.push(("title".to_owned(), title.content)) }
                    if let Some(source) = entry.source { pairs.push(("source".to_owned(), source)) }
                    if let Some(feed_title) = &feed.title { pairs.push(("feed_title".to_owned(), feed_title.content.clone())) }
                    v.push(pairs);
                }
                Ok(v)
            })
    }
}
