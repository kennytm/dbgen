use clap::{
    self,
    builder::{PossibleValuesParser, ValueParser},
    value_parser, ArgAction, Command,
};
use data_encoding::HEXLOWER_PERMISSIVE;
use rand::{rngs::OsRng, RngCore as _};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ffi::OsString};

#[derive(Deserialize, Debug, Clone, PartialEq)]
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
    fn arg_action(&self) -> ArgAction {
        match self {
            Self::Bool => ArgAction::SetTrue,
            Self::Choices { multiple: true, .. } => ArgAction::Append,
            _ => ArgAction::Set,
        }
    }

    fn value_parser(&self) -> ValueParser {
        fn specialized_parse_size(s: &str) -> Result<u64, parse_size::Error> {
            parse_size::parse_size(s)
        }

        match self {
            Self::Bool => ValueParser::bool(),
            Self::Str => ValueParser::string(),
            Self::Int => value_parser!(u64).into(),
            Self::Size => ValueParser::new(specialized_parse_size),
            Self::Float => value_parser!(f64).into(),
            Self::Choices { choices, .. } => ValueParser::new(PossibleValuesParser::new(choices)),
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
    fn to_clap_app(&self) -> Command {
        use clap::builder::{OsStr, Resettable};

        Command::new(&self.name)
            .bin_name(format!("dbdbgen {}", self.name))
            .version(&self.version)
            .about(&self.about)
            .no_binary_name(true)
            .next_line_help(true)
            .args(self.args.iter().map(|(name, arg)| {
                clap::Arg::new(name)
                    .long(if arg.long.is_empty() { name } else { &arg.long })
                    .help(&arg.help)
                    .short(arg.short.chars().next())
                    .action(arg.r#type.arg_action())
                    .value_parser(arg.r#type.value_parser())
                    .required(arg.required)
                    .default_value(Resettable::from(arg.default.as_ref().map(OsStr::from)))
            }))
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
            macro_rules! get_one {
                ($ty:ty) => {
                    if let Some(value) = matches.get_one(name) {
                        let value: &$ty = value;
                        value.clone()
                    } else {
                        continue;
                    }
                };
            }

            let value = match &arg.r#type {
                ArgType::Bool => Match::Bool(matches.get_flag(name)),
                ArgType::Str | ArgType::Choices { multiple: false, .. } => Match::Str(get_one!(String)),
                ArgType::Int | ArgType::Size => Match::Int(get_one!(u64)),
                ArgType::Float => Match::Float(get_one!(f64)),
                ArgType::Choices { multiple: true, .. } => {
                    if let Some(values) = matches.get_many(name) {
                        Match::Array(values.cloned().collect())
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
