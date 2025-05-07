use libsysinspect::SysinspectError;
use libsysinspect::cfg::mmconf::MasterConfig;
use log::Level;
use log::Log;
use log::Record;
use opentelemetry_appender_log::OpenTelemetryLogBridge;
use opentelemetry_otlp::Compression;
use opentelemetry_otlp::LogExporter;
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithTonicConfig;
use opentelemetry_sdk::logs::{BatchLogProcessor, SdkLoggerProvider};
use std::io;
use tokio::sync::OnceCell;

pub mod cycaggr;
pub mod expr;
pub mod logevt;
pub mod mnaggr;

static OTEL_LOGGER: OnceCell<OpenTelemetryLogBridge<SdkLoggerProvider, opentelemetry_sdk::logs::SdkLogger>> =
    OnceCell::const_new();

/// Initialises the OpenTelemetry connection to the collector.
pub async fn init_otel_collector(cfg: MasterConfig) -> Result<(), SysinspectError> {
    let exporter = LogExporter::builder()
        .with_tonic()
        .with_protocol(Protocol::Grpc)
        .with_compression(if cfg.otlp_compression().eq("gzip") { Compression::Gzip } else { Compression::Zstd })
        .with_endpoint(cfg.otlp_collector_endpoint())
        .build()
        .expect("failed to build OTLP exporter");

    let otel_logger: OpenTelemetryLogBridge<SdkLoggerProvider, opentelemetry_sdk::logs::SdkLogger> = OpenTelemetryLogBridge::new(
        &SdkLoggerProvider::builder().with_log_processor(BatchLogProcessor::builder(exporter).build()).build(),
    );

    OTEL_LOGGER
        .set(otel_logger)
        .map_err(|_| SysinspectError::DynError(Box::new(io::Error::new(io::ErrorKind::Other, "Collector already initialized"))))

    /*
    otel_logger.log(
        &Record::builder()
            .args(format_args!("This is my one-off OTLP log"))
            .level(Level::Info)
            .target("manual")
            .module_path_static(Some(module_path!()))
            .file_static(Some(file!()))
            .line(Some(line!()))
            .build(),
    );

    let jpl = json!({
        "user": "alice",
        "action": "login",
        "success": true,
        "items": [1, 2, 3],
    })
    .to_string();

    otel_logger.log(
        &Record::builder()
            .args(format_args!("{}", jpl))
            .level(Level::Info)
            .target("my-app")
            .module_path_static(Some(module_path!()))
            .file_static(Some(file!()))
            .line(Some(line!()))
            .build(),
    );

    Ok(())
    */
}

// Returns a reference to the global OtlpLogger instance.
pub fn otel_logger() -> &'static OpenTelemetryLogBridge<SdkLoggerProvider, opentelemetry_sdk::logs::SdkLogger> {
    OTEL_LOGGER.get().expect("OTEL logger was not initialised")
}

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
