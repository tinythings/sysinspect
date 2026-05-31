use std::{fs, sync::Arc};

use crate::{
    cfg::mmconf::MinionConfig,
    mdescr::catalog::ModelCatalog,
};

fn write_model(dir: &std::path::Path, body: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("model.cfg"), body).unwrap();
}

#[test]
fn catalog_two_valid_models_and_one_broken() {
    let root = tempfile::TempDir::new().unwrap();
    let root_path = root.path();

    // models_dir() returns root_dir().join("models")
    let models_root = root_path.join("models");
    fs::create_dir_all(&models_root).unwrap();

    // Valid model A
    write_model(
        &models_root.join("alpha"),
        r#"
name: Alpha
version: "1.0"
description: First model.
maintainer: a <a@a.a>
"#,
    );

    // Valid model B
    write_model(
        &models_root.join("beta"),
        r#"
name: Beta
version: "2.0"
description: Second model.
maintainer: b <b@b.b>
"#,
    );

    // Broken model
    fs::create_dir_all(models_root.join("broken")).unwrap();
    fs::write(models_root.join("broken").join("model.cfg"), "{{{ bad yaml").unwrap();

    // Non-model directory (no model.cfg) — should be skipped
    fs::create_dir_all(models_root.join("not-a-model")).unwrap();
    fs::write(models_root.join("not-a-model").join("readme.txt"), "nope").unwrap();

    let mut cfg = MinionConfig::default();
    cfg.set_root_dir(root_path.to_str().unwrap());

    let catalog = ModelCatalog::scan(Arc::new(cfg));

    let all = catalog.entries();
    assert_eq!(all.len(), 3, "should discover alpha, beta, broken — not 'not-a-model'");

    let successes = catalog.successes();
    assert_eq!(successes.len(), 2);
    let names: Vec<&str> = successes.iter().map(|m| m.metadata.name.as_str()).collect();
    assert!(names.contains(&"Alpha"));
    assert!(names.contains(&"Beta"));

    let failures = catalog.failures();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].id, "broken");
    assert!(failures[0].result.is_err());
}
