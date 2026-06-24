use super::profiles::{ResolvedModule, group_modules_by_models};
use libsysinspect::console::ConsoleModelRow;

#[test]
fn groups_modules_under_matching_models() {
    let models = vec![
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
    let modules = vec![
        ResolvedModule { name: "module.one".to_string(), version: "1".to_string(), descr: String::new(), selector: "module.one".to_string() },
        ResolvedModule { name: "foo.bar".to_string(), version: "1".to_string(), descr: String::new(), selector: "foo.bar".to_string() },
        ResolvedModule { name: "orphan.mod".to_string(), version: "1".to_string(), descr: String::new(), selector: "orphan.mod".to_string() },
    ];

    let (groups, ungrouped) = group_modules_by_models(&models, &modules);

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].name, "Model A");
    assert_eq!(groups[0].modules.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["module.one"]);
    assert_eq!(groups[1].name, "Model B");
    assert_eq!(groups[1].modules.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["module.one", "foo.bar"]);
    assert_eq!(ungrouped.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["orphan.mod"]);
}
