# Cargo-Bounds

Cargo bounds is a tool to verify dependency ranges in rust, this will let you specify bounds across major versions without worry* and also has tools for finding the most flexible bounds. 

This is extra useful for catching cases of a feature/pr using features of a dependency that dont exist on all versions in the current bound.

Install with
```bash
cargo install cargo-bounds
```

# Disclaimer
**This tool uses `cargo check` or a custom supplied test command to verify if a version works, this relies on your test suit being able to catch any issues. Therefore while `cargo bounds minimize` is a good tool, you might consider only taking its recommendation down to the previous minor version. Some dependencies might have subtle changes between major versions that your tests dont catch.

This tool might also flag a version as incompatible due to some rare corner cases where rust actually wont let you duplicate a crate version, which will lead to this tool flagging it as failed, but the bound isnt actually a issue. It does still point to a area of improvement where you might want to raise the minimum bound on a dependency. 

# Usage

## Test Ranges
run 
```bash
cargo bounds test
```
You might see a result like:
```
toml_edit - ^0.22.10
  0.22.10 FAILED
  0.22.24 OK
Error: 1 deps have failing versions in their bounds. (1 versions failed in total)
```

This will test every major version in the bounds, for example:
```
owo-colors - >=1.0.0, <5
  1.0.0 OK
  2.0.0 OK
  3.0.0 OK
  4.0.0 OK
  4.2.0 OK
```
You can also use a custom check command and run your unit tests for even better coverage
```bash
cargo bounds test --command "cargo test"
```

See `cargo bounds test --help` for more options.

## Minimize dependency
By default 
```bash
cargo bounds minimize
```
Will minimize every dependency, addmitedly this creates a bit of a mess of output so its recommended to instead minimize one at a time:
```bash
cargo bounds minimize your_dependency
```

This always uses `cargo check`, so it is recommended that if you have a test suit to use `cargo bounds test --command "..."` after you update the bounds to verify they work.

### Sanity check
Because this uses a binary search across major versions it might for example find a bound like `>=0.9, <=0.19` as the most flexible, but actually your code fails on `0.15.0` and `0.16.0` (This is actually a real problem I ran into for one of my projects).
These cases should be caught by a `cargo bounds test` run afterwards, but this command also performs a simple `cargo check` across all minor versions in the found bound, you can skip this using the `--skip-sanity` flag
