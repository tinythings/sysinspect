use colored::Colorize;
use libcommon::SysinspectError;
use libsysinspect::cfg::mmconf::CFG_OTLP_COMPRESSION;
use libsysinspect::cfg::mmconf::MasterConfig;
use opentelemetry::Key;
use opentelemetry::logs::AnyValue;
use opentelemetry::logs::Severity;
use opentelemetry::logs::{LogRecord, Logger};
use opentelemetry::{InstrumentationScope, KeyValue, logs::LoggerProvider};
use opentelemetry_otlp::Compression;
use opentelemetry_otlp::LogExporter;
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithTonicConfig;
use opentelemetry_sdk::logs::SdkLogger;
use opentelemetry_sdk::{
    Resource,
    logs::{BatchLogProcessor, SdkLoggerProvider},
};
use serde_json::Value;
use std::collections::HashMap;
use std::{io, time::SystemTime};
use tokio::sync::OnceCell;

pub mod expr;
pub mod logevt;
pub mod query;

static OTEL_LOGGER: OnceCell<SdkLogger> = OnceCell::const_new();

pub async fn init_otel_collector(cfg: MasterConfig) -> Result<(), SysinspectError> {
    if !cfg.telemetry_enabled() {
        log::info!("{} Skipping initialization", "OpenTelemetry is disabled in configuration.".yellow());
        return Ok(());
    }

    let exporter = LogExporter::builder()
        .with_tonic()
        .with_protocol(Protocol::Grpc)
        .with_compression(if cfg.otlp_compression().eq(CFG_OTLP_COMPRESSION) { Compression::Gzip } else { Compression::Zstd })
        .with_endpoint(cfg.otlp_collector_endpoint())
        .build()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let mut resources: Vec<KeyValue> = vec![];
    for (k, v) in cfg.otlp_cfg().resources() {
        resources.push(KeyValue::new(k, v));
    }
    let resource = Resource::builder_empty().with_attributes(resources).build();

    let mut scopedata: Vec<KeyValue> = vec![];
    for (k, v) in cfg.otlp_cfg().scope() {
        scopedata.push(KeyValue::new(k, v));
    }

    let scope = InstrumentationScope::builder("model").with_attributes(scopedata).build();
    let logger = SdkLoggerProvider::builder()
        .with_resource(resource)
        .with_log_processor(BatchLogProcessor::builder(exporter).build())
        .build()
        .logger_with_scope(scope);
    OTEL_LOGGER.set(logger).map_err(|_| SysinspectError::DynError(Box::new(io::Error::other("Collector already initialized"))))?;

    log::info!("Telemetry collector initialized");

    Ok(())
}

// Emit a JSON log record to the OpenTelemetry collector.
pub fn otel_log_json(msg: &Value, attributes: Vec<(String, Value)>) {
    fn json2av(v: &Value) -> AnyValue {
        match v {
            Value::Null => AnyValue::String("null".to_string().into()),
            Value::Bool(b) => AnyValue::Boolean(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    AnyValue::Int(i)
                } else if let Some(f) = n.as_f64() {
                    AnyValue::Double(f)
                } else {
                    AnyValue::String(n.to_string().into())
                }
            }
            Value::String(s) => AnyValue::String(s.clone().into()),
            Value::Array(arr) => AnyValue::ListAny(Box::new(arr.iter().map(json2av).collect())),
            Value::Object(map) => {
                let mut hm: HashMap<Key, AnyValue> = HashMap::new();
                for (k, v) in map {
                    hm.insert(Key::from(k.clone()), json2av(v));
                }
                AnyValue::Map(Box::new(hm))
            }
        }
    }
    let logger = OTEL_LOGGER.get().expect("otel logger must be initialized before use");
    let mut rec = logger.create_log_record();
    rec.set_body(json2av(msg));
    rec.set_severity_number(Severity::Info);
    rec.set_timestamp(SystemTime::now());
    rec.set_observed_timestamp(SystemTime::now());

    for (k, v) in attributes {
        rec.add_attribute(k, json2av(&v));
    }

    rec.set_severity_text("info");

    logger.emit(rec);
}
