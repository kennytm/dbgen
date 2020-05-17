use anyhow::Error;
use dbgen::cli::{run, Args};
use diff::{lines, Result as DiffResult};
use serde_json::from_reader;
use std::{
    fs::{read, read_dir, remove_file, File},
    path::Path,
    str::from_utf8,
};
use tempfile::tempdir;

#[test]
fn run_test() {
    main().unwrap();
}

fn main() -> Result<(), Error> {
    let out_dir = tempdir()?;

    let data_dir = Path::new(file!()).with_file_name("data");
    let zoneinfo_dir = Path::new(file!()).with_file_name("zoneinfo");
    for child_dir in read_dir(data_dir)? {
        let child_dir = child_dir?;
        if !child_dir.file_type()?.is_dir() {
            continue;
        }

        let child_path = child_dir.path();
        eprintln!("Running {}...", child_path.display());
        let mut args: Args = from_reader(File::open(child_path.join("flags.json"))?)?;
        args.template = child_path.join("template.sql");
        args.out_dir = out_dir.path().to_owned();
        args.zoneinfo = zoneinfo_dir.clone();

        run(args)?;

        for result_entry in read_dir(out_dir.path())? {
            let result_entry = result_entry?;
            let expected_path = child_path.join(result_entry.file_name());
            let actual_path = result_entry.path();
            eprintln!("Comparing {} vs {} ...", expected_path.display(), actual_path.display());
            let expected_content = read(expected_path)?;
            let actual_content = read(&actual_path)?;
            if expected_content != actual_content {
                if let (Ok(expected_string), Ok(actual_string)) =
                    (from_utf8(&expected_content), from_utf8(&actual_content))
                {
                    for diff in lines(&expected_string, &actual_string) {
                        match diff {
                            DiffResult::Left(missing) => {
                                eprintln!("\x1b[31m- {}\x1b[0m", missing);
                            }
                            DiffResult::Right(unexpected) => {
                                eprintln!("\x1b[32m+ {}\x1b[0m", unexpected);
                            }
                            DiffResult::Both(same, _) => {
                                eprintln!("  {}", same);
                            }
                        }
                    }
                }
                panic!("CONTENT DIFFERED");
            }
            remove_file(actual_path)?;
        }
    }

    Ok(())
}
