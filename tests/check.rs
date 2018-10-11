use diff::{lines, Result as DiffResult};
use serde_json::from_reader;
use std::{
    env,
    fs::{read_dir, read_to_string, File},
    io::{Error, ErrorKind},
    path::Path,
    process::Command,
};
use tempfile::tempdir;

#[test]
fn run_test() {
    main().unwrap();
}

fn main() -> Result<(), Error> {
    let cargo = env::var_os("CARGO").ok_or_else(|| Error::new(ErrorKind::NotFound, "$CARGO is undefined"))?;
    let out_dir = tempdir()?;

    let data_dir = Path::new(file!()).with_file_name("data");
    for child_dir in read_dir(data_dir)? {
        let child_dir = child_dir?;
        if !child_dir.file_type()?.is_dir() {
            continue;
        }

        let child_path = child_dir.path();

        let mut cmd = Command::new(&cargo);
        cmd.args(&["run", "--", "-i"]).arg(child_path.join("template.sql"));
        cmd.arg("-o").arg(out_dir.path());
        let flags = from_reader::<_, Vec<String>>(File::open(child_path.join("flags.json"))?)?;
        cmd.args(&flags);
        let status = cmd.status()?;
        assert!(
            status.success(),
            "returned invalid status on {}: {}",
            child_path.display(),
            status
        );

        for result_entry in read_dir(out_dir.path())? {
            let result_entry = result_entry?;
            let expected_path = child_path.join(result_entry.file_name());
            let actual_path = result_entry.path();
            eprintln!("Comparing {} vs {} ...", expected_path.display(), actual_path.display());
            let expected_content = read_to_string(expected_path)?;
            let actual_content = read_to_string(actual_path)?;
            if expected_content != actual_content {
                for diff in lines(&expected_content, &actual_content) {
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
                panic!("CONTENT DIFFERED");
            }
        }
    }

    Ok(())
}
