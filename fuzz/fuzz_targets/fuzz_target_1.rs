#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
extern crate dbgen;
extern crate tempfile;

use std::env;
use std::fs::write;
use dbgen::cli::{Args, run, RngName};
use tempfile::tempdir;

fuzz_target!(|data: &[u8]| {
    if data.len() < 32 {
        return;
    }
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&data[..32]);

    env::set_var("DBGEN_WRITE_TO_DEV_NULL", "1");

    let out_dir = tempdir().unwrap();
    let template_path = out_dir.path().join("template");
    write(&template_path, &data[32..]).unwrap();

    drop(run(Args {
        qualified: false,
        table_name: None,
        out_dir: out_dir.path().to_owned(),
        files_count: 5,
        inserts_count: 3,
        rows_count: 6,
        template: template_path,
        seed: Some(seed),
        jobs: 0,
        rng: RngName::Hc128,
        quiet: true,
    }));
});
