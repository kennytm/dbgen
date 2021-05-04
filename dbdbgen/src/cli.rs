use data_encoding::HEXLOWER_PERMISSIVE;
use parse_size::parse_size;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ffi::OsString};

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ArgType {
    Bool,
    Str,
    Int,
    Size,
    Float,
    Choices { choices: Vec<String>, multiple: bool },
}

impl ArgType {
    fn parse_input(&self, input: &str) -> Result<Match, String> {
        match self {
            Self::Int => match input.parse() {
                Ok(u) => Ok(Match::Int(u)),
                Err(e) => Err(e.to_string()),
            },
            Self::Float => match input.parse() {
                Ok(f) => Ok(Match::Float(f)),
                Err(e) => Err(e.to_string()),
            },
            Self::Size => match parse_size(input) {
                Ok(u) => Ok(Match::Int(u)),
                Err(e) => Err(e.to_string()),
            },
            _ => Ok(Match::Str(input.to_owned())),
        }
    }
}

impl Default for ArgType {
    fn default() -> Self {
        Self::Str
    }
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Arg {
    pub short: String,
    pub long: String,
    pub help: String,
    pub required: bool,
    pub default: Option<String>,
    pub r#type: ArgType,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct App {
    pub name: String,
    pub version: String,
    pub about: String,
    pub args: HashMap<String, Arg>,
}

#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum Match {
    Bool(bool),
    Str(String),
    Int(u64),
    Float(f64),
    Array(Vec<String>),
}

pub type Matches<'a> = HashMap<&'a str, Match>;

impl App {
    /// Constructs the clap App from this simplified specification.
    fn to_clap_app(&self) -> clap::App<'_, '_> {
        let mut app = clap::App::new(&self.name)
            .bin_name(format!("dbdbgen {}", self.name))
            .version(&*self.version)
            .about(&*self.about)
            .settings(&[
                clap::AppSettings::NoBinaryName,
                clap::AppSettings::UnifiedHelpMessage,
                clap::AppSettings::NextLineHelp,
            ]);

        for (name, arg) in &self.args {
            let mut clap_arg = clap::Arg::with_name(name)
                .long(if arg.long.is_empty() { name } else { &arg.long })
                .help(&arg.help);
            if !arg.short.is_empty() {
                clap_arg = clap_arg.short(&arg.short);
            }
            match &arg.r#type {
                ArgType::Bool => {}
                ArgType::Str => {
                    clap_arg = clap_arg.takes_value(true);
                }
                ArgType::Int | ArgType::Float | ArgType::Size => {
                    let t = arg.r#type.clone();
                    clap_arg = clap_arg
                        .takes_value(true)
                        .validator(move |s| t.parse_input(&s).map(drop));
                }
                ArgType::Choices { choices, multiple } => {
                    for choice in choices {
                        clap_arg = clap_arg.possible_value(choice);
                    }
                    if *multiple {
                        clap_arg = clap_arg.required(true).use_delimiter(true).multiple(true);
                    }
                }
            }
            if arg.required {
                clap_arg = clap_arg.required(true);
            }
            if let Some(default) = &arg.default {
                clap_arg = clap_arg.default_value(default);
            }

            app = app.arg(clap_arg);
        }

        app
    }

    /// Obtains the matches from the command line.
    pub fn get_matches<I>(&self, args: I) -> Matches<'_>
    where
        I: IntoIterator,
        I::Item: Into<OsString> + Clone,
    {
        let clap_app = self.to_clap_app();
        let matches = clap_app.get_matches_from(args);
        let mut result = HashMap::with_capacity(self.args.len());
        for (name, arg) in &self.args {
            let value = match &arg.r#type {
                ArgType::Bool => Match::Bool(matches.is_present(name)),
                ArgType::Choices { multiple: true, .. } => {
                    if let Some(values) = matches.values_of(name) {
                        Match::Array(values.map(String::from).collect())
                    } else {
                        continue;
                    }
                }
                _ => {
                    if let Some(value) = matches.value_of(name) {
                        arg.r#type.parse_input(value).unwrap()
                    } else {
                        continue;
                    }
                }
            };
            result.insert(&**name, value);
        }
        result
    }
}

pub fn ensure_seed(matches: &mut Matches<'_>) {
    matches.entry("seed").or_insert_with(|| {
        let mut buf = [0u8; 32];
        OsRng.fill_bytes(&mut buf);
        Match::Str(HEXLOWER_PERMISSIVE.encode(&buf))
    });
}
