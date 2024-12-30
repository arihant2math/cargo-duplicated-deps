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
    let mut current = package_map.get(package.name.as_str()).unwrap().into_iter().find(|info| info.version == package.version.to_string()).unwrap();
    loop {
        let next = current.users.iter().find(|user| {
            if let Some(info) = package_map.get(user.name.as_str()) {
                if info.iter().any(|info| info.version == user.version.to_string()) {
                    current = package_map.get(user.name.as_str()).unwrap().into_iter().find(|info| info.version == user.version.to_string()).unwrap();
                    chain.push(format!("{} ({})", user.name.as_str(), user.version.to_string()));
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

fn main() {
    let lockfile = Lockfile::load("Cargo.lock").unwrap();
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
    let mut keys: Vec<String> = package_map.keys().map(|s| s.clone()).collect();
    keys.sort();
    for key in keys {
        let value = package_map.get(key.as_str()).unwrap();
        if value.len() > 1 {
            // Find the latest version
            let mut latest = Version::parse(&value[0].version).unwrap();
            for info in value.iter() {
                let info_version = Version::parse(&info.version).unwrap();
                if info_version > latest {
                    latest = info_version;
                }
            }
            for info in value.iter() {
                if Version::parse(&info.version).unwrap() != latest {
                    println!("{} ({}) {} packages", key, info.version, info.users.len());
                    for user in info.users.iter() {
                        println!("  - {}", get_usage_chain(&package_map, user));
                    }
                }
            }
        }
    }
}
