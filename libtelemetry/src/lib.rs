use libsysinspect::SysinspectError;
use libsysinspect::cfg::mmconf::MasterConfig;
use log::Level;
use log::Log;
use log::Record;
use opentelemetry::logs::Severity;
use opentelemetry::logs::{LogRecord, Logger};

use opentelemetry::{InstrumentationScope, KeyValue, logs::LoggerProvider};
use opentelemetry_appender_log::OpenTelemetryLogBridge;
use opentelemetry_otlp::Compression;
use opentelemetry_otlp::LogExporter;

use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithTonicConfig;
use opentelemetry_sdk::{
    Resource,
    logs::{BatchLogProcessor, SdkLoggerProvider},
};
use std::{io, time::SystemTime};
use tokio::sync::OnceCell;

pub mod aggregate;
pub mod expr;
pub mod logevt;
pub mod query;

static OTEL_LOGGER: OnceCell<OpenTelemetryLogBridge<SdkLoggerProvider, opentelemetry_sdk::logs::SdkLogger>> = OnceCell::const_new();

/// Initialises the OpenTelemetry connection to the collector.
pub async fn init_otel_collector(cfg: MasterConfig) -> Result<(), SysinspectError> {
    let exporter = LogExporter::builder()
        .with_tonic()
        .with_protocol(Protocol::Grpc)
        .with_compression(if cfg.otlp_compression().eq("gzip") { Compression::Gzip } else { Compression::Zstd })
        .with_endpoint(cfg.otlp_collector_endpoint())
        .build()
        .expect("failed to build OTLP exporter");

    let resource = Resource::builder_empty()
        .with_attributes(vec![KeyValue::new("service.name", "my-service"), KeyValue::new("environment", "production")])
        .build();

    let otel_logger: OpenTelemetryLogBridge<SdkLoggerProvider, opentelemetry_sdk::logs::SdkLogger> = OpenTelemetryLogBridge::new(
        &SdkLoggerProvider::builder().with_resource(resource).with_log_processor(BatchLogProcessor::builder(exporter).build()).build(),
    );

    ////////////////
    let exporter = LogExporter::builder()
        .with_tonic()
        .with_protocol(Protocol::Grpc)
        .with_compression(if cfg.otlp_compression().eq("gzip") { Compression::Gzip } else { Compression::Zstd })
        .with_endpoint(cfg.otlp_collector_endpoint())
        .build()
        .expect("failed to build OTLP exporter");
    let resource = Resource::builder_empty()
        .with_attributes(vec![KeyValue::new("service.name", "my-service"), KeyValue::new("environment", "production")])
        .build();
    let scope = InstrumentationScope::builder("my-scope").with_attributes([KeyValue::new("foo", "bar")]).build();
    let logger = &SdkLoggerProvider::builder()
        .with_resource(resource)
        .with_log_processor(BatchLogProcessor::builder(exporter).build())
        .build()
        .logger_with_scope(scope);
    let mut rec = logger.create_log_record();
    rec.set_body("body-data".into());
    rec.set_severity_number(Severity::Info);
    rec.add_attribute("my-attribute", "my-value");
    rec.set_timestamp(SystemTime::now());

    logger.emit(rec);

    ////////////////

    OTEL_LOGGER
        .set(otel_logger)
        .map_err(|_| SysinspectError::DynError(Box::new(io::Error::new(io::ErrorKind::Other, "Collector already initialized"))))
}

// Returns a reference to the global OtlpLogger instance.
pub fn otel_logger() -> &'static OpenTelemetryLogBridge<SdkLoggerProvider, opentelemetry_sdk::logs::SdkLogger> {
    OTEL_LOGGER.get().expect("OTEL logger was not initialised")
}

/// Logs a JSON message to the OpenTelemetry collector.
pub fn otel_log_json(msg: &serde_json::Value) {
    let logger = otel_logger();
    logger.log(
        &Record::builder()
            .args(format_args!("{}", msg))
            .level(Level::Info)
            .target("manual")
            .module_path_static(Some(module_path!()))
            .file_static(Some(file!()))
            .line(Some(line!()))
            .build(),
    );
}

/// Logs a string message to the OpenTelemetry collector.
pub fn otel_log(msg: &str) {
    let logger = otel_logger();
    logger.log(
        &Record::builder()
            .args(format_args!("{}", msg))
            .level(Level::Info)
            .target("manual")
            .module_path_static(Some(module_path!()))
            .file_static(Some(file!()))
            .line(Some(line!()))
            .build(),
    );
}
