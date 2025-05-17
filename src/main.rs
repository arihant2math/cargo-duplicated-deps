use std::collections::HashMap;
use std::fmt::Display;
use std::io::{stdout, IsTerminal};
use std::path::PathBuf;
use std::str::FromStr;
use anyhow::bail;
use cargo_lock::{Lockfile, Package};
use clap::{Parser, ValueEnum};
use crossterm::execute;
use crossterm::style::{SetForegroundColor, ResetColor, Color, Print};
use reqwest::Client;
use semver::Version;
use serde::{Deserialize, Serialize};

async fn get_latest_version(client: &Client, package: &str) -> anyhow::Result<String> {
    let url = format!("https://crates.io/api/v1/crates/{package}");
    let response = client.execute(client.get(&url).build()?).await?;
    let json: serde_json::Value = response.json().await?;
    let latest_version = json["crate"]["newest_version"].as_str().ok_or(anyhow::anyhow!("No version found"))?;
    Ok(latest_version.to_string())
}

#[derive(Clone, Debug)]
struct PackageInfo {
    version: String,
    users: Vec<Package>
}

fn get_usage_chain(package_map: &HashMap<String, Vec<PackageInfo>>, package: &Package) -> String {
    let mut chain = vec![format!("{} v{}", package.name.as_str(), package.version.to_string())];
    let mut current = package_map.get(package.name.as_str()).unwrap().iter().find(|info| info.version == package.version.to_string()).unwrap();
    loop {
        let next = current.users.iter().find(|user| {
            if let Some(info) = package_map.get(user.name.as_str()) {
                if info.iter().any(|info| info.version == user.version.to_string()) {
                    current = package_map.get(user.name.as_str()).unwrap().iter().find(|info| info.version == user.version.to_string()).unwrap();
                    chain.push(format!("{} v{}", user.name.as_str(), user.version));
                    true
                } else {
                    false
                }
            } else {
                false
            }
        });
        if next.is_none() {
            break;
        }
    }
    chain.join(" -> ")
}

#[derive(Clone, Debug, ValueEnum)]
enum Output {
    #[default]
    Text,
    Json,
}

impl Display for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Output::Text => write!(f, "text"),
            Output::Json => write!(f, "json"),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Duplicate {
    pub package: String,
    pub version: String,
    pub latest: String,
    pub users: Vec<Package>
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Response {
    pub duplicates: Vec<Duplicate>
}

#[derive(Parser)]
struct Arguments {
    _call: Option<String>,
    #[arg(short, long)]
    path: Option<PathBuf>,
    #[arg(short, long)]
    color: Option<bool>,
    #[arg(long)]
    offline: bool,
    #[arg(short, long)]
    verbose: bool,
    #[arg(long, default_value_t = Output::Text)]
    output: Output,
}

#[tokio::main]
async fn run() -> anyhow::Result<()> {
    color_eyre::install().map_err(|e| anyhow::anyhow!(e))?;
    let args = Arguments::parse();
    let path = args.path.unwrap_or_else(|| PathBuf::from("Cargo.lock"));
    if args.verbose {
        println!("Reading lockfile from {}", path.display());
    }
    if !path.exists() {
        bail!("{} does not exist", path.display());
    }

    let lockfile = Lockfile::from_str(&tokio::fs::read_to_string(path).await?)?;

    let mut package_map: HashMap<String, Vec<PackageInfo>> = HashMap::new();

    // Pass 1: insert package versions
    for package in &lockfile.packages {
        let info = PackageInfo {
            version: package.version.to_string(),
            users: vec![]
        };
        if let Some(s) = package_map.get_mut(package.name.as_str()) {
            s.push(info);
        } else {
            package_map.insert(package.name.to_string(), vec![info]);
        }
    }

    // Pass 2: insert users
    for package in &lockfile.packages {
        for dep in &package.dependencies {
            if let Some(s) = package_map.get_mut(dep.name.as_str()) {
                for info in s.iter_mut() {
                    if info.version == dep.version.to_string() {
                        info.users.push(package.clone());
                    }
                }
            } else {
                println!("ERROR: {} not found", dep.name);
            }
        }
    }

    // sort by package name
    let mut keys: Vec<String> = package_map.keys().cloned().collect();
    keys.sort();
    let mut duplicates = vec![];
    let client = Client::builder().user_agent("cargo-duplicated-deps").build()?;
    for key in keys {
        let value = package_map.get(key.as_str()).unwrap();
        if value.len() > 1 {
            // Find the latest version
            let default_version = value.iter().max_by_key(|info| Version::parse(&info.version).unwrap()).unwrap().version.clone();
            let latest = if args.offline {
                default_version.clone()
            } else {
                get_latest_version(&client, &key).await.unwrap_or(default_version.clone())
            };
            let default_version = Version::parse(&default_version)?;
            let latest = Version::parse(&latest)?;

            for info in value {
                if Version::parse(&info.version)? != default_version {
                    let mut dup_info = Duplicate {
                        package: key.clone(),
                        version: info.version.clone(),
                        latest: latest.to_string(),
                        users: vec![],
                    };
                    for user in &info.users {
                        dup_info.users.push(user.clone());
                    }
                    duplicates.push(dup_info);
                }
            }
        }
    }

    if let Output::Json = args.output {
        let response = Response {
            duplicates
        };
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        let color = args.color.unwrap_or(stdout().is_terminal());
        for duplicate in duplicates {
            let package_text = if duplicate.users.len() == 1 {
                "package"
            } else {
                "packages"
            };
            if color {
                execute!(
                            stdout(),
                            SetForegroundColor(Color::DarkCyan),
                            Print(&duplicate.package),
                            Print(" "),
                            ResetColor,
                            Print(format!("v{}", duplicate.version)),
                            Print(" "),
                            Print("used by"),
                            Print(" "),
                            Print(duplicate.users.len()),
                            Print(" "),
                            Print(package_text),
                            Print(" "),
                            SetForegroundColor(Color::DarkYellow),
                            Print(format!("(available: v{})", duplicate.latest)),
                            ResetColor,
                        )?;
                println!();
            } else {
                println!("{} v{} used by {} {package_text} (available: v{})", duplicate.package, duplicate.version, duplicate.users.len(), duplicate.latest);
            }
            for user in &duplicate.users {
                println!("  - {}", get_usage_chain(&package_map, user));
            }
        }
    }

    Ok(())
}

fn main() {
    run().unwrap();
}
