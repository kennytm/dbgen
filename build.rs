use vergen::{generate_cargo_keys, ConstantsFlags};

fn main() {
    let flags = ConstantsFlags::SHA | ConstantsFlags::TARGET_TRIPLE | ConstantsFlags::REBUILD_ON_HEAD_CHANGE;
    generate_cargo_keys(flags).unwrap();
}
