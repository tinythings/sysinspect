use std::{fs, sync::Arc};

use crate::{
    cfg::mmconf::MinionConfig,
    mdescr::{
        browse_types::{BrowsedEntrypoint, ModelBrowseDiagnosticLevel},
        browser::ModelBrowser,
    },
};

fn write_model(dir: &tempfile::TempDir, body: &str) {
    fs::write(dir.path().join("model.cfg"), body).unwrap();
}

#[test]
fn load_minimal_model_returns_metadata() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Test Model
version: "1.0"
description: A minimal test model.
maintainer: tester <t@t.t>
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let md = browser.metadata();

    assert_eq!(md.name, "Test Model");
    assert_eq!(md.version, "1.0");
    assert_eq!(md.description, "A minimal test model.");
    assert_eq!(md.maintainer, "tester <t@t.t>");
    assert!(!md.id.is_empty());
    assert_eq!(md.path, td.path().canonicalize().unwrap());
}

#[test]
fn broken_model_returns_load_error() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(&td, "not valid {{{ yaml");

    let result = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path());
    assert!(result.is_err());

    let err = result.unwrap_err().to_string();
    assert!(err.contains("Model load error"));
}

#[test]
fn nonexistent_model_path_returns_load_error() {
    let result = ModelBrowser::load(Arc::new(MinionConfig::default()), std::path::Path::new("/tmp/sysinspect-nonexistent-model-xyz"));
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Model load error"));
}

// Entities

#[test]
fn entities_from_model_with_multiple_entities_and_claims() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Entity Test
version: "0.1"
description: Model with entities.
maintainer: tester <t@t.t>

entities:
  simple-entity:
    descr: A plain entity with no claims.

  rich-entity:
    descr: An entity with claims and dependencies.
    claims:
      $:
        - default:
            key: value
      verbose:
        - detail:
            path: /usr/bin/foo
    inherits:
      - simple-entity
    depends:
      - other-entity
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (entities, _diags) = browser.entities();

    assert_eq!(entities.len(), 2);

    let simple = entities.iter().find(|e| e.id == "simple-entity").expect("simple-entity missing");
    assert_eq!(simple.descr, "A plain entity with no claims.");
    assert!(simple.inherits.is_empty());
    assert!(simple.depends.is_empty());
    assert!(simple.claim_state_keys.is_empty());
    assert!(simple.claim_labels.is_empty());

    let rich = entities.iter().find(|e| e.id == "rich-entity").expect("rich-entity missing");
    assert_eq!(rich.descr, "An entity with claims and dependencies.");
    assert_eq!(rich.inherits, vec!["simple-entity"]);
    assert_eq!(rich.depends, vec!["other-entity"]);
    // Outer claim-state keys
    assert!(rich.claim_state_keys.contains(&"$".to_string()));
    assert!(rich.claim_state_keys.contains(&"verbose".to_string()));
    // Inner claim labels (namespace keys within each claim)
    assert!(rich.claim_labels.contains(&"default".to_string()));
    assert!(rich.claim_labels.contains(&"detail".to_string()));
}

#[test]
fn entities_empty_when_no_entities_section() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: No Entities Model
version: "0.1"
description: Model without entities section.
maintainer: tester <t@t.t>
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (entities, _diags) = browser.entities();
    assert!(entities.is_empty());
}

#[test]
fn entity_with_empty_descr_returns_empty_string() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Empty Descr Test
version: "0.1"
description: Entity with no descr.
maintainer: tester <t@t.t>

entities:
  bare: {}
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (entities, _diags) = browser.entities();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].id, "bare");
    assert_eq!(entities[0].descr, "");
}

#[test]
fn entity_claims_with_non_dollar_labels() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Claims Test
version: "0.1"
description: Entity with multiple claim labels.
maintainer: tester <t@t.t>

entities:
  multi-claim:
    descr: Entity with labels.
    claims:
      baseline:
        - data:
            key: val1
      verbose:
        - data:
            key: val2
      $:
        - data:
            key: val3
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (entities, _diags) = browser.entities();
    assert_eq!(entities.len(), 1);

    let state_keys = &entities[0].claim_state_keys;
    assert_eq!(state_keys.len(), 3);
    assert!(state_keys.contains(&"$".to_string()));
    assert!(state_keys.contains(&"baseline".to_string()));
    assert!(state_keys.contains(&"verbose".to_string()));

    let inner_labels = &entities[0].claim_labels;
    assert_eq!(inner_labels.len(), 1);
    assert!(inner_labels.contains(&"data".to_string()));
}

// Relations & Checkbook

#[test]
fn relations_from_model_with_checkbook_and_relations() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Relations Test
version: "0.1"
description: Model with relations and checkbook.
maintainer: tester <t@t.t>

checkbook:
  os-check:
    - os-info
    - net-info

relations:
  os-info:
    $:
      requires:
        - general-info
        - packages-info
  net-info:
    $:
      requires:
        - routing-info
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");

    let (relations, _diags) = browser.relations();
    assert_eq!(relations.len(), 2);

    let os_info = relations.iter().find(|r| r.id == "os-info").expect("os-info missing");
    assert_eq!(os_info.states.len(), 1);
    let st = &os_info.states[0];
    assert_eq!(st.state, "$");
    assert_eq!(st.required_entities, vec!["general-info", "packages-info"]);

    let (entrypoints, _diags) = browser.entrypoints();
    let cb = entrypoints
        .iter()
        .find_map(|ep| match ep {
            BrowsedEntrypoint::CheckbookLabel { label, .. } if label == "os-check" => Some(ep),
            _ => None,
        })
        .expect("os-check entrypoint missing");

    match cb {
        BrowsedEntrypoint::CheckbookLabel { label, relation_ids, entity_ids } => {
            assert_eq!(label, "os-check");
            assert_eq!(relation_ids.len(), 2);
            assert!(relation_ids.contains(&"os-info".to_string()));
            assert!(relation_ids.contains(&"net-info".to_string()));
            assert!(entity_ids.contains(&"general-info".to_string()));
            assert!(entity_ids.contains(&"packages-info".to_string()));
            assert!(entity_ids.contains(&"routing-info".to_string()));
        }
        _ => unreachable!(),
    }
}

#[test]
fn checkbook_missing_relation_emits_diagnostic() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Missing Rel Test
version: "0.1"
description: Model referencing a relation that does not exist.
maintainer: tester <t@t.t>

checkbook:
  broken-label:
    - ghost-rel
relations:
  real-rel:
    $:
      requires:
        - some-entity
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (_entrypoints, diagnostics) = browser.entrypoints();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].level, ModelBrowseDiagnosticLevel::Warning);
    assert!(diagnostics[0].message.contains("ghost-rel"));
    assert!(diagnostics[0].message.contains("broken-label"));
}

#[test]
fn checkbook_without_relations_gives_entity_entrypoints_only() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: No Relations
version: "0.1"
description: Model with checkbook but no relations.
maintainer: tester <t@t.t>

checkbook:
  my-label:
    - missing-rel

entities:
  e1:
    descr: An entity
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (entrypoints, diagnostics) = browser.entrypoints();

    let has_entity = entrypoints.iter().any(|ep| matches!(ep, BrowsedEntrypoint::Entity { id, .. } if id == "e1"));
    assert!(has_entity, "entity entrypoint must be present");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("missing-rel"));
}

#[test]
fn relations_with_non_dollar_state() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Non-Dollar Rel Test
version: "0.1"
description: Relations with named states.
maintainer: tester <t@t.t>

relations:
  my-rel:
    $:
      requires:
        - e1
    verbose:
      requires:
        - e2
        - e3
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (relations, _diags) = browser.relations();
    assert_eq!(relations.len(), 1);
    assert_eq!(relations[0].id, "my-rel");
    assert_eq!(relations[0].states.len(), 2);

    let dollar_st = relations[0].states.iter().find(|s| s.state == "$").expect("$ state missing");
    assert_eq!(dollar_st.required_entities, vec!["e1"]);

    let verbose_st = relations[0].states.iter().find(|s| s.state == "verbose").expect("verbose state missing");
    assert_eq!(verbose_st.required_entities, vec!["e2", "e3"]);
}

// Actions & States

#[test]
fn actions_with_multiple_states() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Multi-State Test
version: "0.1"
description: Model with actions having multiple states.
maintainer: tester <t@t.t>

entities:
  e1:
    descr: Test entity

actions:
  multi-state-action:
    description: An action with named states.
    module: sys.run
    bind:
      - e1
    state:
      $:
        args:
          cmd: "default"
      verbose:
        args:
          cmd: "verbose mode"
      bootstrap:
        args:
          cmd: "bootstrap mode"
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (actions, _diags) = browser.actions();

    assert_eq!(actions.len(), 1);
    let action = &actions[0];
    assert_eq!(action.action_id, "multi-state-action");
    assert_eq!(action.description, "An action with named states.");
    assert_eq!(action.module, "sys.run");
    assert_eq!(action.binds_to, vec!["e1"]);
    assert_eq!(action.states.len(), 3);

    let state_names: Vec<&str> = action.states.iter().map(|s| s.state.as_str()).collect();
    assert!(state_names.contains(&"$"));
    assert!(state_names.contains(&"verbose"));
    assert!(state_names.contains(&"bootstrap"));
}

#[test]
fn named_state_only_actions_are_not_lost() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Named-State-Only Test
version: "0.1"
description: Model with actions that have NO default $ state.
maintainer: tester <t@t.t>

entities:
  e1:
    descr: Test entity

actions:
  only-named:
    description: Action with only named states, no $.
    module: cfg.resource
    bind:
      - e1
    state:
      bootstrap:
        opts:
          - push
        args:
          src: /some/path
      regen-priv:
        opts:
          - push
        args:
          src: /other/path
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (actions, _diags) = browser.actions();

    assert_eq!(actions.len(), 1, "named-state-only action must be present");
    let action = &actions[0];
    assert_eq!(action.action_id, "only-named");
    assert_eq!(action.states.len(), 2);
    assert!(!action.states.iter().any(|s| s.state == "$"), "action should NOT have a $ state");

    let state_names: Vec<&str> = action.states.iter().map(|s| s.state.as_str()).collect();
    assert!(state_names.contains(&"bootstrap"));
    assert!(state_names.contains(&"regen-priv"));
}

// Params

#[test]
fn action_state_params_opts_args_ctx_conds() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Params Test
version: "0.1"
description: Model with full action state parameters.
maintainer: tester <t@t.t>

entities:
  e1:
    descr: Test entity

actions:
  full-action:
    description: Action with opts, args, ctx, and conds.
    module: sys.proc
    bind:
      - e1
    state:
      $:
        opts:
          - limits
          - pid
        args:
          search: "/sbin/init"
          started: true
        ctx:
          search: "Process search mask"
          timeout: "Max wait in seconds"
        conds:
          uid: 65432
          working-dir: "/tmp"
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (actions, _diags) = browser.actions();

    assert_eq!(actions.len(), 1);
    let action = &actions[0];
    assert_eq!(action.states.len(), 1);

    let st = &action.states[0];
    assert_eq!(st.state, "$");
    assert_eq!(st.opts, vec!["limits", "pid"]);
    assert_eq!(st.args.len(), 2);
    assert!(st.args.contains(&("search".to_string(), "/sbin/init".to_string())));
    assert!(st.args.contains(&("started".to_string(), "true".to_string())));
    assert_eq!(st.context_vars.len(), 2);
    assert!(st.context_vars.contains(&("search".to_string(), "Process search mask".to_string())));
    assert!(st.context_vars.contains(&("timeout".to_string(), "Max wait in seconds".to_string())));
    assert_eq!(st.conditions.len(), 2);
    assert!(st.conditions.contains(&("uid".to_string(), "65432".to_string())));
    assert!(st.conditions.contains(&("working-dir".to_string(), "/tmp".to_string())));
}

#[test]
fn missing_ctx_gives_empty_context_vars() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: No Ctx Test
version: "0.1"
description: Model without ctx declarations.
maintainer: tester <t@t.t>

entities:
  e1:
    descr: Test entity

actions:
  no-ctx:
    description: Action with no ctx.
    module: sys.run
    bind:
      - e1
    state:
      $:
        args:
          cmd: "uname -a"
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (actions, _diags) = browser.actions();

    assert_eq!(actions.len(), 1);
    let st = &actions[0].states[0];
    assert!(st.context_vars.is_empty(), "ctx should be empty when not declared");
}

#[test]
fn args_preserve_template_values() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Template Args Test
version: "0.1"
description: Args containing claim() and context() template functions.
maintainer: tester <t@t.t>

entities:
  e1:
    descr: Test entity

actions:
  tmpl:
    description: Action with template args.
    module: sys.proc
    bind: [e1]
    state:
      $:
        args:
          search: "claim(common.path)"
          timeout: "{{ context.timeout | default(value='30') }}"
          flag: true
          count: 42
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (actions, _diags) = browser.actions();
    let st = &actions[0].states[0];

    // claim() strings survive Tera rendering unchanged
    assert!(st.args.contains(&("search".to_string(), "claim(common.path)".to_string())));
    // Tera templates are rendered during mspec::load(); context.timeout is undefined
    // so the default filter produces "30".
    assert!(st.args.contains(&("timeout".to_string(), "30".to_string())));
    assert!(st.args.contains(&("flag".to_string(), "true".to_string())));
    assert!(st.args.contains(&("count".to_string(), "42".to_string())));
}

// States

#[test]
fn states_union_includes_dollar_and_named() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: States Test
version: "0.1"
description: Model with mixed states.
maintainer: tester <t@t.t>

entities:
  e1:
    descr: Test entity

actions:
  a1:
    description: First action with $ state.
    module: sys.run
    bind:
      - e1
    state:
      $:
        args:
          cmd: "default"
  a2:
    description: Second action with named states.
    module: sys.proc
    bind:
      - e1
    state:
      bootstrap:
        args:
          search: "init"
      verbose:
        args:
          search: "init"
        opts:
          - pid
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let states = browser.states();

    assert_eq!(states.len(), 3);
    assert!(states.contains(&"$".to_string()), "must include $");
    assert!(states.contains(&"bootstrap".to_string()));
    assert!(states.contains(&"verbose".to_string()));
    assert_eq!(states, vec!["$", "bootstrap", "verbose"]);
}

#[test]
fn states_empty_when_no_actions() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: No Actions
version: "0.1"
description: Model without any actions.
maintainer: tester <t@t.t>
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    assert!(browser.states().is_empty());
}

#[test]
fn states_deduplicates_across_actions() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Dedup Test
version: "0.1"
description: Duplicate state names across actions.
maintainer: tester <t@t.t>

entities:
  e1:
    descr: Test entity

actions:
  a1:
    description: Action 1
    module: sys.run
    bind: [e1]
    state:
      verbose:
        args:
          cmd: "one"
  a2:
    description: Action 2
    module: sys.run
    bind: [e1]
    state:
      verbose:
        args:
          cmd: "two"
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    assert_eq!(browser.states(), vec!["verbose"]);
}

// Diagnostics

#[test]
fn malformed_entity_produces_diagnostic_not_silent_omission() {
    // Non-mapping entity bodies now produce a diagnostic, and the entity
    // still appears in the output with default values (id + empty descr).
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Malformed Entity Test
version: "0.1"
description: One entity is broken among valid ones.
maintainer: tester <t@t.t>

entities:
  good:
    descr: A valid entity.
  bad:
    - this
    - is
    - a
    - sequence
    - not
    - a
    - mapping
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (entities, diagnostics) = browser.entities();

    // Both are extracted; the "bad" one falls back to defaults.
    assert_eq!(entities.len(), 2);
    assert_eq!(entities[0].id, "good");
    assert_eq!(entities[0].descr, "A valid entity.");
    assert_eq!(entities[1].id, "bad");
    assert_eq!(entities[1].descr, "");

    // A diagnostic warns about the non-mapping body.
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].level, ModelBrowseDiagnosticLevel::Warning);
    assert!(diagnostics[0].message.contains("bad"));
    assert!(diagnostics[0].message.contains("not a mapping"));
}

#[test]
fn malformed_action_produces_diagnostic_not_silent_omission() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Malformed Action Test
version: "0.1"
description: One action is broken among valid ones.
maintainer: tester <t@t.t>

entities:
  e1:
    descr: Test entity

actions:
  good:
    description: A valid action.
    module: sys.run
    bind: [e1]
    state:
      $:
        args:
          cmd: "ok"
  bad:
    - this is not a mapping
    - so it cannot deserialize
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (actions, diagnostics) = browser.actions();

    // Good action is still extracted
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].action_id, "good");

    // Bad action produced a diagnostic
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].level, ModelBrowseDiagnosticLevel::Warning);
    assert!(diagnostics[0].message.contains("bad"));
    assert!(diagnostics[0].message.contains("could not be parsed"));
}

#[test]
fn malformed_relation_produces_diagnostic_not_silent_omission() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Malformed Rel Test
version: "0.1"
description: One relation is broken among valid ones.
maintainer: tester <t@t.t>

relations:
  good-rel:
    $:
      requires:
        - e1
  bad-rel: [not, a, mapping]
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (relations, diagnostics) = browser.relations();

    // Good relation is still extracted
    assert_eq!(relations.len(), 1);
    assert_eq!(relations[0].id, "good-rel");

    // Bad relation produced a diagnostic
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].level, ModelBrowseDiagnosticLevel::Warning);
    assert!(diagnostics[0].message.contains("bad-rel"));
    assert!(diagnostics[0].message.contains("could not be parsed"));
}

#[test]
fn action_binds_to_unknown_entity_emits_diagnostic() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Unknown Bind Test
version: "0.1"
description: Action binds to an entity not in the model.
maintainer: tester <t@t.t>

entities:
  e1:
    descr: Known entity.

actions:
  stray:
    description: Binds to something that does not exist.
    module: sys.run
    bind:
      - e1
      - ghost-entity
    state:
      $:
        args:
          cmd: "test"
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let (actions, diagnostics) = browser.actions();

    // Action is still present
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].action_id, "stray");

    // Diagnostic for the unknown bind
    let diag = diagnostics.iter().find(|d| d.message.contains("ghost-entity")).expect("diagnostic for ghost-entity missing");
    assert_eq!(diag.level, ModelBrowseDiagnosticLevel::Warning);
    assert!(diag.message.contains("ghost-entity"));
    assert!(diag.message.contains("stray"));
}

// summarize()

#[test]
fn summarize_returns_complete_browsed_model() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Full Model
version: "2.0"
description: A complete model for summary testing.
maintainer: tester <t@t.t>

checkbook:
  main-check:
    - info-rel

relations:
  info-rel:
    $:
      requires:
        - sys-info
        - pkg-info

entities:
  sys-info:
    descr: System information entity.
    claims:
      $:
        - default:
            key: system
  pkg-info:
    descr: Package information entity.
    inherits:
      - sys-info

actions:
  machine-id:
    description: Display machine-id.
    module: sys.run
    bind:
      - sys-info
    state:
      $:
        args:
          cmd: "cat /etc/machine-id"
  os-version:
    description: Display OS version.
    module: sys.run
    bind:
      - sys-info
    state:
      $:
        args:
          cmd: "uname -a"
      verbose:
        args:
          cmd: "uname -a"
        opts:
          - pid
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let summary = browser.summarize().expect("summarize should succeed");

    // Metadata
    assert_eq!(summary.metadata.name, "Full Model");
    assert_eq!(summary.metadata.version, "2.0");

    // Entities
    assert_eq!(summary.entities.len(), 2);
    let sys_info = summary.entities.iter().find(|e| e.id == "sys-info").expect("sys-info missing");
    assert_eq!(sys_info.descr, "System information entity.");
    assert_eq!(sys_info.claim_state_keys, vec!["$"]);

    // Relations
    assert_eq!(summary.relations.len(), 1);
    assert_eq!(summary.relations[0].id, "info-rel");

    // Entrypoints
    assert!(summary.entrypoints.iter().any(|ep| matches!(ep,
        BrowsedEntrypoint::CheckbookLabel { label, .. } if label == "main-check"
    )));
    assert!(summary.entrypoints.iter().any(|ep| matches!(ep,
        BrowsedEntrypoint::Entity { id, .. } if id == "sys-info"
    )));

    // Actions
    assert_eq!(summary.actions.len(), 2);
    let os_ver = summary.actions.iter().find(|a| a.action_id == "os-version").expect("os-version missing");
    assert_eq!(os_ver.states.len(), 2);
    assert!(os_ver.states.iter().any(|s| s.state == "$"));
    assert!(os_ver.states.iter().any(|s| s.state == "verbose"));

    // States
    assert!(summary.states.contains(&"$".to_string()));
    assert!(summary.states.contains(&"verbose".to_string()));

    // No diagnostics expected for this valid model
    assert!(summary.diagnostics.is_empty());
}

// Smoke test against real example

#[test]
fn keypair_demo_smoke_test() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/demos/keypair");
    assert!(p.exists(), "keypair demo directory not found at {}", p.display());

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), &p).expect("keypair demo should load");

    let summary = browser.summarize().expect("keypair demo should summarize");

    // Basic smoke assertions
    assert!(!summary.metadata.name.is_empty());
    assert_eq!(summary.entities.len(), 1);
    assert_eq!(summary.actions.len(), 4, "keypair demo has 4 actions");
    assert!(!summary.entrypoints.is_empty());

    // Verify named states are captured (the demo uses bootstrap, regen-priv, repair-pub)
    let regen = summary.actions.iter().find(|a| a.action_id == "keypair-regen-priv").expect("keypair-regen-priv action missing");
    assert!(regen.states.iter().any(|s| s.state == "regen-priv"));
}

// Hardening

#[test]
fn malformed_entity_body_produces_warning_not_silent_default() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Malformed Body Test
version: "0.1"
description: Entity with a non-mapping body.
maintainer: tester <t@t.t>

entities:
  good:
    descr: A valid entity.
  bad:
    - this
    - is
    - a
    - sequence
    - not
    - a
    - mapping
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let summary = browser.summarize().expect("summarize should succeed");

    // Both entities appear in output; the bad one has defaults.
    assert_eq!(summary.entities.len(), 2);
    let bad = summary.entities.iter().find(|e| e.id == "bad").expect("bad entity still present");
    assert_eq!(bad.descr, "");

    // A warning diagnostic was emitted for the non-mapping body.
    let diag = summary
        .diagnostics
        .iter()
        .find(|d| d.message.contains("bad") && d.message.contains("not a mapping"))
        .expect("diagnostic for non-mapping entity body missing");
    assert_eq!(diag.level, ModelBrowseDiagnosticLevel::Warning);
}

#[test]
fn relation_referencing_missing_entity_produces_diagnostic() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Missing Entity Ref Test
version: "0.1"
description: Relation requires an entity that does not exist.
maintainer: tester <t@t.t>

entities:
  e1:
    descr: The only entity.

relations:
  bad-rel:
    $:
      requires:
        - e1
        - ghost-entity
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let summary = browser.summarize().expect("summarize should succeed");

    let diag = summary
        .diagnostics
        .iter()
        .find(|d| d.message.contains("ghost-entity") && d.message.contains("bad-rel"))
        .expect("diagnostic for unknown entity in relation missing");
    assert_eq!(diag.level, ModelBrowseDiagnosticLevel::Warning);
}

#[test]
fn summarize_deduplicates_diagnostics() {
    let td = tempfile::TempDir::new().unwrap();
    write_model(
        &td,
        r#"
name: Dedup Test
version: "0.1"
description: Model where entrypoints re-extraction would normally duplicate diagnostics.
maintainer: tester <t@t.t>

checkbook:
  my-label:
    - my-rel

relations:
  my-rel:
    $:
      requires:
        - e1

entities:
  e1:
    descr: An entity.

actions:
  a1:
    description: An action.
    module: sys.run
    bind: [e1]
    state:
      $:
        args:
          cmd: "test"

  stray-action:
    description: Binds to unknown entity.
    module: sys.run
    bind: [ghost-entity]
    state:
      $:
        args:
          cmd: "test"
"#,
    );

    let browser = ModelBrowser::load(Arc::new(MinionConfig::default()), td.path()).expect("load should succeed");
    let summary = browser.summarize().expect("summarize should succeed");

    // There should be exactly one diagnostic for stray-action's unknown bind,
    // not multiple copies from overlapping extractions.
    let ghost_diags: Vec<_> = summary.diagnostics.iter().filter(|d| d.message.contains("ghost-entity")).collect();
    assert_eq!(ghost_diags.len(), 1, "diagnostic for ghost-entity should appear exactly once, found {}", ghost_diags.len());
}
