use crate::{
    cli::{App, Matches},
    error::{Error, Purpose},
};
use dbgen::cli::Args;
use jsonnet::JsonnetVm;
use serde::Deserialize;
use std::{ffi::OsStr, fs::read_to_string, path::Path};

pub struct Vm<'p> {
    vm: JsonnetVm,
    path: &'p OsStr,
}

fn deserialize<'a, T: Deserialize<'a>>(js: &'a str, purpose: Purpose) -> Result<T, Error> {
    serde_json::from_str(js).map_err(|error| {
        use std::fmt::Write;

        let mut src = String::new();
        let end_line = error.line();
        let start_line = end_line.saturating_sub(5);
        for (line, line_num) in js.lines().skip(start_line).zip(start_line..end_line) {
            writeln!(&mut src, "{:5} | {}", line_num + 1, line).unwrap();
        }
        src.push_str(&" ".repeat(7 + error.column()));
        src.push('^');

        Error::Serde { purpose, src, error }
    })
}

impl<'p> Vm<'p> {
    pub fn new(path: &'p OsStr, allow_import: bool) -> Result<Self, Error> {
        let content = read_to_string(path)?;
        let mut vm = JsonnetVm::new();
        vm.import_callback(|_, base, rel| {
            if rel == Path::new("dbdbgen.libsonnet") {
                Ok((rel.to_owned(), include_str!("../dbdbgen.libsonnet").to_owned()))
            } else if allow_import {
                let path = base.join(rel);
                let text = read_to_string(&path).map_err(|e| e.to_string())?;
                Ok((path, text))
            } else {
                Err("external import is disabled".to_owned())
            }
        });
        vm.tla_code("src", &content);
        Ok(Self { vm, path })
    }

    pub fn eval_arguments(&mut self) -> Result<App, Error> {
        let app_js = self
            .vm
            .evaluate_snippet(self.path, "function(src) {[k]: src[k] for k in ['name', 'version', 'about', 'args']}")
            .map_err(|error| Error::Jsonnet {
                purpose: Purpose::Arguments,
                message: error.to_string(),
            })?;
        let app = deserialize(&app_js, Purpose::Arguments)?;
        Ok(app)
    }

    pub fn eval_steps(&mut self, matches: Matches<'_>) -> Result<Vec<Args>, Error> {
        let matches_js = serde_json::to_string(&matches).unwrap();
        self.vm.tla_code("matches", &matches_js);

        let steps_js_stream = self
            .vm
            .evaluate_snippet_stream(
                self.path,
                "function(src, matches) if std.isArray(src.steps) then src.steps else src.steps(matches)",
            )
            .map_err(|error| Error::Jsonnet {
                purpose: Purpose::Execution { step: 0 },
                message: error.to_string(),
            })?;

        let mut steps = Vec::new();
        for (i, steps_js) in steps_js_stream.iter().enumerate() {
            let step = deserialize(&steps_js, Purpose::Execution { step: i })?;
            steps.push(step);
        }

        Ok(steps)
    }
}
