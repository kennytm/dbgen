use vergen::{vergen, Config};

fn main() {
    let mut cfg = Config::default();
    let git = cfg.git_mut();
    *git.branch_mut() = false;
    *git.commit_timestamp_mut() = false;
    *git.semver_mut() = false;
    let cargo = cfg.cargo_mut();
    *cargo.features_mut() = false;
    *cargo.profile_mut() = false;

    vergen(cfg).unwrap();
}
