mod mspec_error_diagnostics_test {
    use libcommon::SysinspectError;
    use libsysinspect::{
        cfg::mmconf::MinionConfig,
        mdescr::mspec,
        tmpl::render::ModelTplRender,
    };
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn write_model(td: &TempDir, body: &str) {
        fs::write(td.path().join("model.cfg"), body).unwrap();
    }

    #[test]
    fn template_compile_error_reports_stage_and_template_name() {
        let mut r = ModelTplRender::new("model.cfg", r#"x: "{{ traits["a.b"] | default("x") }}""#);
        let e = r.render().unwrap_err().to_string();

        assert!(e.contains("Template compile failed for \"model.cfg\""));
        assert!(e.contains("cause:"));
    }

    #[test]
    fn load_reports_render_failure_with_full_model_path() {
        let td = TempDir::new().unwrap();
        write_model(
            &td,
            r#"
name: bad
version: "0.1"
actions:
  x:
    module: sys.run
    bind: [e]
    state:
      $:
        args:
          cmd: "{{ traits["a.b"] | default("x") }}"
"#,
        );

        let err = mspec::load(Arc::new(MinionConfig::default()), td.path().to_str().unwrap(), None, None).unwrap_err();
        let msg = err.to_string();

        assert!(msg.contains("Unable to render template"));
        assert!(msg.contains(td.path().to_str().unwrap()));
        assert!(msg.contains("Template compile failed"));
    }

    #[test]
    fn load_reports_yaml_line_column_and_source_line() {
        let td = TempDir::new().unwrap();
        write_model(
            &td,
            r#"
name: bad-yaml
version: "0.1"
description: [
"#,
        );

        let err = mspec::load(Arc::new(MinionConfig::default()), td.path().to_str().unwrap(), None, None).unwrap_err();
        let msg = err.to_string();

        assert!(msg.contains("Unable to parse \""));
        assert!(msg.contains("line"));
        assert!(msg.contains("column"));
        assert!(msg.contains("\n  > "));
    }

    #[test]
    fn keypair_demo_model_parses_after_default_filter_fix() {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/demos/keypair");
        let spec = mspec::load(Arc::new(MinionConfig::default()), p.to_str().unwrap(), None, None);
        if let Err(SysinspectError::ModelDSLError(e)) = &spec {
            panic!("keypair demo should parse, got ModelDSLError: {e}");
        }
        assert!(spec.is_ok());
    }
}
