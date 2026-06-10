//! OpenTelemetry export (M2 observability slice).
//!
//! Env-gated: when `AINM_OTEL__ENDPOINT` is set (an OTLP/HTTP collector base
//! URL, e.g. `http://localhost:4318` — `/v1/traces` is appended), every
//! `tracing` span the server already emits is exported there; unset means the
//! layer is absent and behavior is unchanged.
//! Dashboards (p99, error rate) are built on the collector side once the
//! hosted deploy exists — this module is the wiring, not the dashboards.

use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::registry::LookupSpan;

/// Pure env core: (endpoint, service_name) or `None` when telemetry is off.
pub fn settings_with(get: &dyn Fn(&str) -> Option<String>) -> Option<(String, String)> {
    let endpoint = get("AINM_OTEL__ENDPOINT")?;
    let endpoint = endpoint.trim().to_string();
    if endpoint.is_empty() {
        return None;
    }
    let service = get("AINM_OTEL__SERVICE_NAME").unwrap_or_else(|| "ainm-server".to_string());
    Some((endpoint, service))
}

/// Build the OTLP tracing layer for `endpoint`. Lazy: no collector connection
/// is attempted until the first batch export.
pub fn build_layer<S>(
    endpoint: &str,
    service_name: &str,
) -> Result<OpenTelemetryLayer<S, opentelemetry_sdk::trace::SdkTracer>, String>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    // with_endpoint() is used verbatim by the HTTP exporter (no signal path is
    // appended for explicit endpoints) — collectors expect POST /v1/traces.
    let endpoint = format!("{}/v1/traces", endpoint.trim_end_matches('/'));
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .build()
        .map_err(|e| format!("OTLP exporter: {e}"))?;
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            Resource::builder()
                .with_attribute(KeyValue::new("service.name", service_name.to_string()))
                .build(),
        )
        .build();
    let tracer = provider.tracer("ainm-server");
    opentelemetry::global::set_tracer_provider(provider);
    Ok(tracing_opentelemetry::layer().with_tracer(tracer))
}

/// Env-gated layer for the production subscriber: `None` (and unchanged
/// behavior) unless `AINM_OTEL__ENDPOINT` is set.
pub fn layer_from_env<S>() -> Option<OpenTelemetryLayer<S, opentelemetry_sdk::trace::SdkTracer>>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    let (endpoint, service) = settings_with(&|k| std::env::var(k).ok())?;
    match build_layer(&endpoint, &service) {
        Ok(layer) => {
            // Can't tracing::info! yet — the subscriber isn't installed.
            eprintln!("[otel] exporting spans to {endpoint} as {service}");
            Some(layer)
        }
        Err(e) => {
            eprintln!("[otel] disabled — failed to build OTLP exporter: {e}");
            None
        }
    }
}

// Silence unused warning for the trait import used only via method syntax.
#[allow(unused_imports)]
use tracing_subscriber as _ts;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_none_when_endpoint_unset_or_blank() {
        assert_eq!(settings_with(&|_| None), None);
        assert_eq!(
            settings_with(&|k| (k == "AINM_OTEL__ENDPOINT").then(|| "  ".into())),
            None
        );
    }

    #[test]
    fn settings_default_service_name_and_override() {
        let got = settings_with(&|k| {
            (k == "AINM_OTEL__ENDPOINT").then(|| "http://localhost:4317".into())
        });
        assert_eq!(
            got,
            Some(("http://localhost:4317".into(), "ainm-server".into()))
        );

        let got = settings_with(&|k| match k {
            "AINM_OTEL__ENDPOINT" => Some("http://c:4317".into()),
            "AINM_OTEL__SERVICE_NAME" => Some("ainm-staging".into()),
            _ => None,
        });
        assert_eq!(got, Some(("http://c:4317".into(), "ainm-staging".into())));
    }

    #[test]
    fn build_layer_constructs_without_a_collector() {
        // Lazy exporter: construction must succeed with nothing listening.
        let layer =
            build_layer::<tracing_subscriber::Registry>("http://127.0.0.1:59999", "ainm-test");
        assert!(layer.is_ok(), "{:?}", layer.err());
    }
}
