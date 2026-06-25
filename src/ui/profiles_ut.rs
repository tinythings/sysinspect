use super::profiles::{ResolvedModule, group_modules_by_models};
use libsysinspect::console::ConsoleModelRow;

#[test]
fn groups_modules_under_profile_models_with_coverage_flags() {
    let model_rows = vec![
        ConsoleModelRow {
            id: "model-a".to_string(),
            enabled: true,
            name: "Model A".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            entrypoints: vec![],
            entrypoint_kinds: vec![],
            public_entrypoints: vec![],
            public_entrypoint_kinds: vec![],
            public_actions: vec![],
            modules: vec!["module.one".to_string(), "module.two".to_string()],
            states: vec![],
            target_actions: vec![],
        },
        ConsoleModelRow {
            id: "model-b".to_string(),
            enabled: true,
            name: "Model B".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            entrypoints: vec![],
            entrypoint_kinds: vec![],
            public_entrypoints: vec![],
            public_entrypoint_kinds: vec![],
            public_actions: vec![],
            modules: vec!["module.one".to_string(), "foo.bar".to_string()],
            states: vec![],
            target_actions: vec![],
        },
    ];
    let resolved_modules = vec![
        ResolvedModule { name: "module.one".to_string(), version: "1".to_string(), descr: String::new(), selector: "module.one".to_string(), covered: true },
        ResolvedModule { name: "foo.bar".to_string(), version: "1".to_string(), descr: String::new(), selector: "foo.bar".to_string(), covered: true },
        ResolvedModule { name: "orphan.mod".to_string(), version: "1".to_string(), descr: String::new(), selector: "orphan.mod".to_string(), covered: true },
    ];
    let profile_model_ids = vec!["model-a".to_string(), "model-b".to_string()];

    let (groups, extras) = group_modules_by_models(&profile_model_ids, &model_rows, &resolved_modules);

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].name, "Model A");
    let g0_names: Vec<&str> = groups[0].modules.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(g0_names, vec!["module.one", "module.two"]);
    assert!(groups[0].modules[0].covered);
    assert!(!groups[0].modules[1].covered);

    assert_eq!(groups[1].name, "Model B");
    let g1_names: Vec<&str> = groups[1].modules.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(g1_names, vec!["module.one", "foo.bar"]);
    assert!(groups[1].modules[0].covered);
    assert!(groups[1].modules[1].covered);

    assert_eq!(extras.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["orphan.mod"]);
}
