use clap::{App, AppSettings, Arg};
use dbdbgen::{cli::ensure_seed, error::Error, jsvm::Vm};
use dbgen::{span::Registry, FULL_VERSION};
use std::error::Error as StdError;

fn run() -> Result<(), Error> {
    let global_matches = App::new("dbdbgen")
        .long_version(FULL_VERSION)
        .setting(AppSettings::TrailingVarArg)
        .args(&[
            Arg::with_name("dry-run")
                .long("dry-run")
                .help("Only display the evaluated dbdbgen result without generating data."),
            Arg::with_name("allow-import")
                .long("allow-import")
                .help("Allows `import` and `importstr` to read files."),
            Arg::with_name("file")
                .help("The Jsonnet file to execute, followed by the arguments passed to it.")
                .multiple(true)
                .required(true)
                .allow_hyphen_values(true),
        ])
        .get_matches();
    let mut args = global_matches.values_of_os("file").unwrap();
    let src_file = args.next().unwrap();

    let mut vm = Vm::new(src_file, global_matches.is_present("allow-import"))?;
    let app = vm.eval_arguments()?;
    let mut matches = app.get_matches(args);
    ensure_seed(&mut matches);
    let steps = vm.eval_steps(matches)?;

    if global_matches.is_present("dry-run") {
        println!(
            "/* dbdbgen{}\n*/\n{{\"steps\": {}}}",
            FULL_VERSION,
            serde_json::to_string_pretty(&steps).unwrap()
        );
        return Ok(());
    }

    let steps_count = steps.len();
    for (step, arg) in steps.into_iter().enumerate() {
        if !arg.quiet {
            eprintln!("step {} / {}", step + 1, steps_count);
        }
        let mut registry = Registry::default();
        dbgen::cli::run(arg, &mut registry).map_err(|e| Error::Dbgen {
            step,
            message: registry.describe(&e),
        })?;
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}\n", e);
        let mut err: &dyn StdError = &e;
        while let Some(source) = err.source() {
            eprintln!("Cause: {}", source);
            err = source;
        }
    }
}
