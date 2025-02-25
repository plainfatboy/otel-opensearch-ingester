use core::error::{ApplicationError, BoxDynError};

use axum::{extract::State, http::StatusCode, Router};
use ::opensearch::{
    OpenSearch,
    auth::Credentials,
    cert::CertificateValidation,
    http::{
        Url,
        transport::{SingleNodeConnectionPool, TransportBuilder},
    },
};
use opensearch::exporter::Exporter;
use opentelemetry_proto::tonic::collector::logs::v1::{
    ExportLogsServiceRequest, ExportLogsServiceResponse,
    logs_service_server::{LogsService, LogsServiceServer},
};
use opentelemetry_sdk::{metrics::SdkMeterProvider, resource::ResourceBuilder, Resource};
use prometheus::{Registry, TextEncoder};
use tokio::task::JoinSet;
use tonic::{codec::CompressionEncoding, transport::Server};

mod config;
mod core;
mod opensearch;
mod utils;

#[derive(Debug)]
pub struct MyServer {
    exporter: Exporter,
}

impl MyServer {
    pub fn new(exporter: Exporter) -> Self {
        Self { exporter }
    }
}

#[tonic::async_trait]
impl LogsService for MyServer {
    async fn export(
        &self,
        request: tonic::Request<ExportLogsServiceRequest>,
    ) -> Result<tonic::Response<ExportLogsServiceResponse>, tonic::Status> {
        match self.exporter.export(request.into_inner()).await {
            Ok(response) => Ok(tonic::Response::new(response)),
            Err(err) => match err {
                ApplicationError::Retryable(e) => Err(tonic::Status::unavailable(e.to_string())),
                ApplicationError::NonRetryable(e) => Err(tonic::Status::internal(e.to_string())),
            },
        }
    }
}

#[derive(Clone)]
struct AxumRequestState {
    opensearch_client: OpenSearch,
    prometheus_registry: Registry,
}

async fn healthz(State(state): State<AxumRequestState>) -> StatusCode {
    match state.opensearch_client.ping().send().await {
        Ok(resp) => {
            if resp.status_code().is_success() {
                StatusCode::OK
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        },
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn metrics(State(state): State<AxumRequestState>) -> axum::response::Response {
    let encoder = TextEncoder::new();
    let gathered = state.prometheus_registry.gather();
    let content = encoder.encode_to_string(&gathered).unwrap();

    axum::response::Response::builder()
        .header("Content-Type", prometheus::TEXT_FORMAT)
        .body(content.into())
        .unwrap()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), BoxDynError> {
    let config = config::read_config_from_env()?;

    let prometheus_registry = prometheus::Registry::new();
    let meter_provider = {
        let exporter = opentelemetry_prometheus::exporter()
            .with_registry(prometheus_registry.clone())
            .build()?;

        let provider = SdkMeterProvider::builder()
            .with_reader(exporter)
            .with_resource(
                Resource::builder_empty()
                    .with_service_name("otel-opensearch-ingester")
                    .build()
            )
            .build();

        opentelemetry::global::set_meter_provider(provider.clone());
        provider
    };

    let endpoint = Url::parse(&config.opensearch.host)?;
    let pool: SingleNodeConnectionPool = SingleNodeConnectionPool::new(endpoint);
    let transport = {
        let mut b = TransportBuilder::new(pool)
            .disable_proxy()
            .auth(Credentials::Basic(
                config.opensearch.username.clone(),
                config.opensearch.password.clone(),
            ));

        if config.opensearch.tls_insecure_skip_verify {
            b = b.cert_validation(CertificateValidation::None);
        }

        b.build()?
    };
    let client = OpenSearch::new(transport);
    let exporter = Exporter::new(
        client.clone(),
        config.opensearch.index_name,
        config.opensearch.index_with_date_suffix,
    );

    let mut set: JoinSet<Result<(), BoxDynError>> = tokio::task::JoinSet::new();
    set.spawn(async move {
        Server::builder()
            .add_service(
                LogsServiceServer::new(MyServer::new(exporter))
                    .send_compressed(CompressionEncoding::Gzip)
                    .accept_compressed(CompressionEncoding::Gzip)
            )
            .serve(format!("0.0.0.0:{}", config.server.port).parse()?)
            .await?;
        Ok(())
    });
    set.spawn(async move {
        let app = Router::new()
            .route("/metrics", axum::routing::get(metrics))
            .route("/healthz", axum::routing::get(healthz))
            .with_state(
                AxumRequestState{
                    opensearch_client: client,
                    prometheus_registry,
                },
            );
        let listener =
            tokio::net::TcpListener::bind(
                format!("0.0.0.0:{}", config.server.http_port)
            )
            .await?;
        axum::serve(listener, app).await?;
        Ok(())
    });
    let result= set.join_all().await;
    for res in result {
        res?;
    }

    meter_provider.shutdown()?;
    Ok(())
}
