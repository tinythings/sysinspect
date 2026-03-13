use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::process::Command;

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap_or_else(|| panic!("failed to resolve repository root"))
}

fn cargo_metadata() -> Value {
    let out = Command::new("cargo")
        .current_dir(repo_root())
        .args(["metadata", "--format-version", "1", "--locked"])
        .output()
        .unwrap_or_else(|err| panic!("failed to run cargo metadata: {err}"));

    if !out.status.success() {
        panic!("cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr));
    }

    serde_json::from_slice(&out.stdout).unwrap_or_else(|err| panic!("failed to parse cargo metadata output: {err}"))
}

#[test]
fn only_py3_runtime_reaches_rustpython_dependencies() {
    let metadata = cargo_metadata();
    let workspace_members: BTreeSet<String> = metadata["workspace_members"]
        .as_array()
        .unwrap_or_else(|| panic!("missing workspace_members"))
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();

    let mut package_names = BTreeMap::<String, String>::new();
    let mut rustpython_ids = BTreeSet::<String>::new();
    for pkg in metadata["packages"].as_array().unwrap_or_else(|| panic!("missing packages")) {
        let id = pkg["id"].as_str().unwrap_or_else(|| panic!("package without id")).to_string();
        let name = pkg["name"].as_str().unwrap_or_else(|| panic!("package without name")).to_string();
        if name.starts_with("rustpython") {
            rustpython_ids.insert(id.clone());
        }
        package_names.insert(id, name);
    }

    let mut edges = BTreeMap::<String, Vec<String>>::new();
    for node in metadata["resolve"]["nodes"].as_array().unwrap_or_else(|| panic!("missing resolve.nodes")) {
        let id = node["id"].as_str().unwrap_or_else(|| panic!("node without id")).to_string();
        let deps = node["deps"]
            .as_array()
            .unwrap_or_else(|| panic!("node without deps"))
            .iter()
            .filter_map(|dep| dep["pkg"].as_str().map(str::to_string))
            .collect::<Vec<String>>();
        edges.insert(id, deps);
    }

    let mut offenders = Vec::<String>::new();
    let mut py3_runtime_reaches_rustpython = false;

    for member in workspace_members {
        let mut seen = BTreeSet::<String>::new();
        let mut queue = VecDeque::from([member.clone()]);
        let mut reaches_rustpython = false;

        while let Some(id) = queue.pop_front() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if rustpython_ids.contains(&id) {
                reaches_rustpython = true;
                break;
            }
            for dep in edges.get(&id).cloned().unwrap_or_default() {
                queue.push_back(dep);
            }
        }

        let name = package_names.get(&member).cloned().unwrap_or(member.clone());
        if name == "py3-runtime" {
            py3_runtime_reaches_rustpython = reaches_rustpython;
        } else if reaches_rustpython {
            offenders.push(name);
        }
    }

    assert!(py3_runtime_reaches_rustpython, "py3-runtime should keep the RustPython dependency");
    assert!(offenders.is_empty(), "unexpected RustPython reachability outside py3-runtime: {offenders:?}");
}

#[test]
fn libsysinspect_and_sysminion_check_without_direct_rustpython_dependency() {
    for pkg in ["libsysinspect", "sysminion"] {
        let out = Command::new("cargo")
            .current_dir(repo_root())
            .args(["check", "-p", pkg, "--quiet"])
            .output()
            .unwrap_or_else(|err| panic!("failed to run cargo check for {pkg}: {err}"));

        if !out.status.success() {
            panic!("cargo check -p {pkg} failed: {}", String::from_utf8_lossy(&out.stderr));
        }
    }
}
