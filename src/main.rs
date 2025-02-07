use std::collections::HashMap;
use cargo_lock::{Lockfile, Package};
use semver::Version;

#[derive(Clone, Debug)]
struct PackageInfo {
    version: String,
    users: Vec<Package>
}

fn get_usage_chain(package_map: &HashMap<String, Vec<PackageInfo>>, package: &Package) -> String {
    let mut chain = vec![format!("{} ({})", package.name.as_str(), package.version.to_string())];
    let mut current = package_map.get(package.name.as_str()).unwrap().iter().find(|info| info.version == package.version.to_string()).unwrap();
    loop {
        let next = current.users.iter().find(|user| {
            if let Some(info) = package_map.get(user.name.as_str()) {
                if info.iter().any(|info| info.version == user.version.to_string()) {
                    current = package_map.get(user.name.as_str()).unwrap().iter().find(|info| info.version == user.version.to_string()).unwrap();
                    chain.push(format!("{} ({})", user.name.as_str(), user.version));
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

#[derive(Clone, Debug)]
enum Output {
    Text,
    Json,
}

impl Default for Output {
    fn default() -> Self {
        Output::Text
    }
}

#[derive(Parser)]
struct Arguments {
    _call: Option<String>,
    #[arg(short, long)]
    path: Option<PathBuf>,
    #[arg(short, long)]
    color: Option<bool>,
    #[arg(short, long)]
    verbose: bool,
    #[arg(short, long, default)]
    output: Output,
}

fn main() -> anyhow::Result<()> {
    color_eyre::install().unwrap();
    let args = Arguments::parse();
    let path = args.path.unwrap_or_else(|| PathBuf::from("Cargo.lock"));
    if args.verbose {
        println!("Reading lockfile from {}", path.display());
    }
    if !path.exists() {
        bail!("{} does not exist", path.display());
    }

    let lockfile = Lockfile::load(path)?;

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
    for key in keys {
        let value = package_map.get(key.as_str()).unwrap();
        if value.len() > 1 {
            // Find the latest version
            let mut latest = Version::parse(&value[0].version).unwrap();
            for info in value {
                let info_version = Version::parse(&info.version).unwrap();
                if info_version > latest {
                    latest = info_version;
                }
            }
            for info in value {
                if Version::parse(&info.version).unwrap() != latest {
                    println!("{} ({}) {} packages", key, info.version, info.users.len());
                    for user in &info.users {
                        println!("  - {}", get_usage_chain(&package_map, user));
                    }
                }
            }
        }
    }
}
