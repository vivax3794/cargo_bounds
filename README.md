# Cargo-Bounds 🎉

**Cargo-Bounds** is an awesome tool designed to help you verify and optimize your dependency ranges in Rust! It lets you specify bounds across major versions with ease and even finds the most flexible constraints, ensuring your project only uses compatible features. Perfect for catching those sneaky cases where a feature might be used from a dependency that doesn’t support it in every version! 😍🚀

---

## Installation

Install Cargo-Bounds with a single command:
```bash
cargo install cargo-bounds
```

---

## Disclaimer ⚠️

- **Test Reliance:**  
  Cargo-Bounds uses `cargo check` (or a custom test command you provide) to verify if a version works. This means it depends on your test **suite** catching any issues. While `cargo bounds minimize` is super helpful, consider its recommendation only down to the previous minor version because subtle changes might slip through! ✨

- **Corner Cases:**  
  In some rare cases, Cargo-Bounds might flag a version as incompatible because Rust won’t let you duplicate a crate version—even if the bound isn’t actually an issue. This still indicates that you might want to raise the minimum bound on that dependency. Always run your full test suite after updating! 💖

---

## Usage

### Testing Dependency Ranges 🧪

To test every major version in your dependency bounds, simply run:
```bash
cargo bounds test
```
*Example output:*
```
toml_edit - ^0.22.10
  0.22.10 FAILED
  0.22.24 OK
Error: 1 deps have failing versions in their bounds. (1 versions failed in total)
```

For another example:
```
owo-colors - >=1.0.0, <5
  1.0.0 OK
  2.0.0 OK
  3.0.0 OK
  4.0.0 OK
  4.2.0 OK
```

Need to run your unit tests for extra confidence? Use a custom check command:
```bash
cargo bounds test --command "cargo test"
```
For more options, check out:
```bash
cargo bounds test --help
```
Isn’t that neat? 😎

---

### Minimizing Dependency Bounds ✂️

To minimize the bounds for all dependencies:
```bash
cargo bounds minimize
```
Since that might produce a bit of output chaos, it’s often better to minimize one dependency at a time:
```bash
cargo bounds minimize your_dependency
```
*Note:* This command always uses `cargo check`. So, it’s a great idea to run:
```bash
cargo bounds test --command "..."
```
after updating to verify that everything still works perfectly! 🌟

#### Sanity Check 🔍

Cargo-Bounds uses a binary search across major versions. For example, it might find a bound like `>=0.9, <=0.19` as the most flexible—even if your code fails on versions like `0.15.0` or `0.16.0` (a real scenario I’ve encountered!). While running `cargo bounds test` should catch these issues, Cargo-Bounds also performs a quick sanity check across all minor versions in the found bound. If needed, you can skip this check using the `--skip-sanity` flag.
