use anyhow::Context;
use std::{
    fmt::{Debug, Display, Formatter},
    fs::File,
    io,
    io::BufReader,
    path::Path,
    str::FromStr,
};

use cron::Schedule;
use derive_more::{Deref, DerefMut, FromStr};
use log::{error, info};
use serde::{Deserialize, Serialize};
use tap::TapFallible;
use toml::{map::Map, Value};

const DEFAULT_TIME_BETWEEN_TICKS: usize = 60000;

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    #[serde(default = "default_time_between_ticks")]
    pub time_between_ticks: usize,
    #[serde(default)]
    pub print_plugin_warnings: bool,
    #[serde(default = "Vec::new")]
    pub sources: Vec<Source>,
    #[serde(default = "Vec::new")]
    pub sinks: Vec<Sink>,
    #[serde(default = "Vec::new")]
    pub processors: Vec<Processor>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Source {
    pub name: String,
    pub schedule: ParsedSchedule,
    pub r#type: String,
    pub pipe: Vec<Pipeline>,
    #[serde(flatten)]
    pub other_fields: Map<String, Value>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Sink {
    pub name: String,
    pub r#type: String,
    #[serde(flatten)]
    pub other_fields: Map<String, Value>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Processor {
    pub name: String,
    pub r#type: String,
    #[serde(flatten)]
    pub other_fields: Map<String, Value>,
}

#[inline]
const fn default_time_between_ticks() -> usize {
    DEFAULT_TIME_BETWEEN_TICKS
}

#[derive(Deserialize, Serialize, Debug, Deref, DerefMut, Clone, FromStr)]
#[serde(try_from = "String", into = "String")]
pub struct ParsedSchedule(pub Schedule);

impl TryFrom<String> for ParsedSchedule {
    type Error = cron::error::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ParsedSchedule::from_str(&value)
    }
}

impl From<ParsedSchedule> for String {
    fn from(value: ParsedSchedule) -> Self {
        value.0.to_string()
    }
}

impl Display for ParsedSchedule {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.to_string().as_str())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(try_from = "String", into = "String")]
pub struct Pipeline {
    pub processors: Vec<String>,
    pub sink: String,
}

pub struct PipelineFromStrError;

impl Debug for PipelineFromStrError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        pipeline_err_fmt(f)
    }
}

impl Display for PipelineFromStrError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        pipeline_err_fmt(f)
    }
}

#[inline]
fn pipeline_err_fmt(f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str("Corrupt pipeline string")
}

impl FromStr for Pipeline {
    type Err = PipelineFromStrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut v = s.split("->").map(ToOwned::to_owned).collect::<Vec<_>>();
        let sink = v.pop().ok_or(PipelineFromStrError)?;
        Ok(Pipeline {
            processors: v,
            sink,
        })
    }
}

impl TryFrom<String> for Pipeline {
    type Error = PipelineFromStrError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        FromStr::from_str(&value)
    }
}

impl Display for Pipeline {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut v = self.processors.join("->");
        v.reserve(2 + self.sink.len());
        v.push_str("->");
        v.push_str(&self.sink);
        f.write_str(&v)
    }
}

impl From<Pipeline> for String {
    fn from(value: Pipeline) -> Self {
        value.to_string()
    }
}

pub fn load_from_toml(config_path: impl AsRef<Path>) -> anyhow::Result<Config> {
    let config_path = config_path.as_ref();
    info!("Loading config {}", config_path.to_string_lossy());
    let reader = BufReader::new(
        File::open(config_path)
            .tap_err(|err| error!("Unable to open file {:?}. Details: {err}", config_path))?,
    );
    let config_toml = io::read_to_string(reader).tap_err(|err| {
        error!(
            "Unable to read config file {:?}. Details: {err}",
            config_path
        )
    })?;
    toml::from_str(&config_toml)
        .tap_err(|err| error!("TOML parsing error: {err}"))
        .context("Parsing toml")
}
