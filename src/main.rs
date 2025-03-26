#![feature(exit_status_error)]

use std::{
    fs,
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{Result, anyhow};
use clap::Parser;
use owo_colors::OwoColorize;
use toml_edit::DocumentMut;

#[derive(Parser, Debug)]
/// Test the current range
struct TestConfig {
    #[arg(short, long)]
    minor: bool,
    #[arg(short, long)]
    patch: bool,
    #[arg(short = 's', long)]
    print_skiped: bool,
    #[arg(short, long)]
    dep: Option<String>,
}

/// Test dep ranges
#[derive(Parser, Debug)]
enum Cli {
    Test(TestConfig),
    Minimize { dep: Option<String> },
}

#[derive(Clone)]
struct State {
    cargo_toml: Box<str>,
}

impl State {
    fn store() -> Result<Self> {
        Ok(State {
            cargo_toml: fs::read_to_string("Cargo.toml")?.into(),
        })
    }

    fn restore(&self) -> Result<()> {
        fs::write("Cargo.toml", self.cargo_toml.as_bytes())?;

        Ok(())
    }
}

impl Drop for State {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let prev_state = State::store()?;
    let cloned_state = prev_state.clone();
    ctrlc::set_handler(move || {
        let _ = cloned_state.restore();
        std::process::exit(1);
    })?;
    let res = main_impl(&prev_state, cli);
    prev_state.restore()?;

    res
}

#[inline]
fn main_impl(state: &State, cli: Cli) -> Result<()> {
    match run_test(format!("{}", "baseline".red()))? {
        TestResult::Fail => return Err(anyhow!("Baseline failed")),
        TestResult::Sucess => {}
    }
    match cli {
        Cli::Test(mut test) => {
            if test.patch {
                test.minor = true;
            }
            let res = sanity_test(state, test)?;
            if res.failed_deps != 0 {
                Err(anyhow!("{}", res.print()))
            } else {
                Ok(())
            }
        }
        Cli::Minimize { dep } => minimize(state, dep),
    }
}

fn minimize(state: &State, dep: Option<String>) -> Result<()> {
    let cargo_toml = state.cargo_toml.parse::<DocumentMut>()?;
    let Some(deps) = cargo_toml.get("dependencies") else {
        println!("{}", "No dependencies".bright_red());
        return Ok(());
    };
    let deps = deps
        .as_table()
        .ok_or(anyhow!("[depdencies] wasnt a table"))?;

    if let Some(dep) = dep {
        if !deps.contains_key(&dep) {
            return Err(anyhow!("dep {dep} not found."));
        }
        minimize_dep(state, &dep)?;
    } else {
        let deps = deps.iter().map(|(key, _)| key).collect::<Vec<_>>();
        for dep in deps {
            minimize_dep(state, dep)?;
        }
    }
    Ok(())
}

fn sanity_test(state: &State, config: TestConfig) -> Result<TestSummary> {
    let cargo_toml = state.cargo_toml.parse::<DocumentMut>()?;
    let Some(deps) = cargo_toml.get("dependencies") else {
        println!("{}", "No dependencies".bright_red());
        return Ok(TestSummary::default());
    };
    let deps = deps
        .as_table()
        .ok_or(anyhow!("[depdencies] wasnt a table"))?;

    let mut summary = TestSummary::default();
    if let Some(dep) = &config.dep {
        if !deps.contains_key(dep) {
            return Err(anyhow!("dep {dep} not found."));
        }
        summary.failed_versions = sanity_test_dep(state, dep, &config)?;
        if summary.failed_versions != 0 {
            summary.failed_deps = 1;
        }
    } else {
        let deps = deps.iter().map(|(key, _)| key).collect::<Vec<_>>();
        for dep in deps {
            let fails = sanity_test_dep(state, dep, &config)?;
            summary.failed_versions += fails;
            if fails != 0 {
                summary.failed_deps += 1;
            }
        }
    }
    Ok(summary)
}

#[derive(Default)]
struct TestSummary {
    failed_deps: u8,
    failed_versions: u16,
}

impl TestSummary {
    fn print(self) -> String {
        format!(
            "{} deps have failing versions in their bounds. ({} versions failed in total)",
            self.failed_deps.red(),
            self.failed_versions.yellow()
        )
    }
}

fn sanity_test_dep(state: &State, dep: &str, config: &TestConfig) -> Result<u16> {
    let mut cargo_toml = state.cargo_toml.parse::<DocumentMut>()?;
    let deps = cargo_toml
        .get_mut("dependencies")
        .unwrap()
        .as_table_mut()
        .unwrap();
    let dep_item = deps.get_mut(dep).unwrap();

    let bound;
    if let Some(ver) = dep_item.as_str() {
        let mut new_table = toml_edit::InlineTable::new();
        new_table.insert("version", ver.into());
        bound = semver::VersionReq::parse(ver)?;
        *dep_item = new_table.into();
    } else {
        let ver = dep_item
            .as_table_like()
            .ok_or(anyhow!("Unexpected dep type"))?;
        let ver = ver.get("version").ok_or(anyhow!("Expected version key"))?;
        let ver = ver.as_str().ok_or(anyhow!("Expected str"))?;
        bound = semver::VersionReq::parse(ver)?;
    }

    println!("{} - {}", dep.blue(), bound.yellow());

    let mut versions = get_versions(dep)?;
    versions.retain(|version| bound.matches(version));
    versions.sort();

    let mut last_major = u64::MAX;
    let mut last_minor = u64::MAX;

    let last_version = versions.last().unwrap().clone();
    let mut fails = 0;
    for version in versions {
        if version.major == last_major
            && (!config.minor || (last_minor == version.minor && !config.patch))
            && version != last_version
        {
            if config.print_skiped {
                println!("  {}", version.bright_black());
            }
            continue;
        }

        last_minor = version.minor;
        last_major = version.major;

        if let TestResult::Fail = test_version(&mut cargo_toml, dep, version)? {
            fails += 1;
        }
    }

    Ok(fails)
}

fn minimize_dep(state: &State, dep: &str) -> Result<()> {
    let mut cargo_toml = state.cargo_toml.parse::<DocumentMut>()?;
    let deps = cargo_toml
        .get_mut("dependencies")
        .unwrap()
        .as_table_mut()
        .unwrap();
    let dep_item = deps.get_mut(dep).unwrap();

    let bound;
    if let Some(ver) = dep_item.as_str() {
        let mut new_table = toml_edit::InlineTable::new();
        new_table.insert("version", ver.into());
        bound = semver::VersionReq::parse(ver)?;
        *dep_item = new_table.into();
    } else {
        let ver = dep_item
            .as_table_like()
            .ok_or(anyhow!("Unexpected dep type"))?;
        let ver = ver.get("version").ok_or(anyhow!("Expected version key"))?;
        let ver = ver.as_str().ok_or(anyhow!("Expected str"))?;
        bound = semver::VersionReq::parse(ver)?;
    }

    println!("{} - {}", dep.blue(), bound.yellow());

    let mut versions = get_versions(dep)?;
    versions.sort();

    let mut current_supported = versions.clone();
    current_supported.retain(|version| bound.matches(version));

    let min_version = current_supported[0].clone();
    let max_version = current_supported.last().unwrap().clone();

    let (min_index, _) = versions
        .iter()
        .enumerate()
        .find(|(_, ver)| **ver == min_version)
        .unwrap();
    let (max_index, _) = versions
        .iter()
        .enumerate()
        .find(|(_, ver)| **ver == max_version)
        .unwrap();

    println!("  Minimizing {}", versions[min_index].yellow());
    let min_version = binary_search(
        &versions[..=min_index],
        &mut cargo_toml,
        dep,
        TestResult::Sucess,
    )?;
    println!("  Found min {}", min_version.green());
    println!("  Maximizing {}", versions[max_index].yellow());
    let max_version = binary_search(
        &versions[max_index..],
        &mut cargo_toml,
        dep,
        TestResult::Fail,
    )?;
    println!("  Found max {}", max_version.green());

    let bound = semver::VersionReq::parse(&format!(">={min_version}, <={max_version}"))?;
    println!("  {}", bound.green());
    Ok(())
}

fn binary_search(
    versions: &[semver::Version],
    cargo_toml: &mut DocumentMut,
    dep: &str,
    upper_kind: TestResult,
) -> Result<semver::Version> {
    let mut low = 0;
    let mut top = versions.len() - 1;

    while top - low > 1 {
        let center = (low + top) / 2;
        let res = test_version(cargo_toml, dep, versions[center].clone())?;

        if res == upper_kind {
            top = center;
        } else {
            low = center;
        }
    }

    let low_res = test_version(cargo_toml, dep, versions[low].clone())?;
    let top_res = test_version(cargo_toml, dep, versions[top].clone())?;

    if low_res == top_res {
        if upper_kind == TestResult::Fail {
            return Ok(versions[top].clone());
        } else {
            return Ok(versions[low].clone());
        }
    }

    if upper_kind == TestResult::Fail {
        Ok(versions[low].clone())
    } else {
        Ok(versions[top].clone())
    }
}

fn test_version(
    cargo_toml: &mut DocumentMut,
    dep: &str,
    version: semver::Version,
) -> Result<TestResult> {
    cargo_toml["dependencies"][dep]["version"] = format!("={version}").into();
    fs::write("Cargo.toml", cargo_toml.to_string())?;
    run_test(version.blue().to_string())
}

fn run_test(msg: String) -> Result<TestResult> {
    let spinner = indicatif::ProgressBar::new_spinner().with_message(msg.clone());
    spinner.enable_steady_tick(Duration::from_millis(100));
    let res = test_inner()?;

    let res_text = match res {
        TestResult::Fail => "FAILED".red().to_string(),
        TestResult::Sucess => "OK".green().to_string(),
    };
    spinner.finish_with_message(format!("{msg} {res_text}"));
    Ok(res)
}

fn test_inner() -> Result<TestResult> {
    match Command::new("cargo")
        .arg("update")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .exit_ok()
    {
        Ok(_) => {}
        Err(_) => return Ok(TestResult::Fail),
    }
    match Command::new("cargo")
        .arg("check")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .exit_ok()
    {
        Ok(_) => Ok(TestResult::Sucess),
        Err(_) => Ok(TestResult::Fail),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TestResult {
    Fail,
    Sucess,
}

fn get_versions(dep: &str) -> Result<Vec<semver::Version>> {
    let spinner = indicatif::ProgressBar::new_spinner()
        .with_message(format!("Fetching versions for {}", dep.blue()));
    spinner.enable_steady_tick(Duration::from_millis(100));

    let client = crates_io_api::SyncClient::new(
        "cargo-bounds (vivax3794@pm.me)",
        Duration::from_millis(1000),
    )?;
    let dep = client.get_crate(dep)?;

    let mut result = Vec::new();
    for version in dep.versions {
        if !version.yanked {
            let ver = semver::Version::parse(&version.num)?;
            if ver.pre.is_empty() {
                result.push(ver);
            }
        }
    }

    spinner.finish_and_clear();
    Ok(result)
}
