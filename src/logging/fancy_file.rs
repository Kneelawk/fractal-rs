use log4rs::{
    append::{file::FileAppender, Append},
    config::{Deserialize, Deserializers},
    encode::EncoderConfig,
};
use regex::Regex;
use std::path::PathBuf;

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FancyFileAppenderConfig {
    path: String,
    encoder: Option<EncoderConfig>,
    append: Option<bool>,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct FancyFileAppenderDeserializer;

impl Deserialize for FancyFileAppenderDeserializer {
    type Trait = dyn Append;

    type Config = FancyFileAppenderConfig;

    fn deserialize(
        &self,
        config: FancyFileAppenderConfig,
        deserializers: &Deserializers,
    ) -> anyhow::Result<Box<Self::Trait>> {
        let mut appender = FileAppender::builder();
        if let Some(append) = config.append {
            appender = appender.append(append);
        }
        if let Some(encoder) = config.encoder {
            appender = appender.encoder(deserializers.deserialize(&encoder.kind, encoder.config)?);
        }
        Ok(Box::new(appender.build(&expand_dates(config.path.into()))?))
    }
}

lazy_static::lazy_static! {
static ref DATE_EXPANSION_PATTERN: Regex = Regex::new(r#"\{(?P<type>\w)(\((?P<args>[^)]+)\))?\}"#).unwrap();
}

fn expand_dates(path: PathBuf) -> PathBuf {
    let mut path = path.to_string_lossy().to_string();

    for c in DATE_EXPANSION_PATTERN.captures_iter(&path.clone()) {
        if let Some(str) = expand_date(
            c.name("type").unwrap().as_str(),
            c.name("args").map(|m| m.as_str()),
        ) {
            path = path.replace(&c[0], &str);
        }
    }

    path.into()
}

fn expand_date(ty: &str, args: Option<&str>) -> Option<String> {
    match ty {
        "d" => Some(
            chrono::Local::now()
                .format(args.unwrap_or("%Y-%m-%d_%H-%M-%S"))
                .to_string(),
        ),
        _ => None,
    }
}
