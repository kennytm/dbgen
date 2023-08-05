use vergen::EmitBuilder;

fn main() {
    EmitBuilder::builder()
        .git_sha(false)
        .cargo_target_triple()
        .emit()
        .unwrap();
}
