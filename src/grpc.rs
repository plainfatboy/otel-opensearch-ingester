use core::error::{ApplicationError, BoxDynError};

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
use tonic::{codec::CompressionEncoding, transport::Server};

mod config;
mod core;
mod opensearch;
mod utils;

#[derive(Debug, Default)]
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), BoxDynError> {
    let config = config::read_config_from_env()?;
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
        client,
        config.opensearch.index_name,
        config.opensearch.index_with_date_suffix,
    );

    Server::builder()
        .add_service(
            LogsServiceServer::new(MyServer::new(exporter))
                .send_compressed(CompressionEncoding::Gzip)
                .accept_compressed(CompressionEncoding::Gzip)
        )
        .serve(format!("0.0.0.0:{}", config.server.port).parse()?)
        .await?;

    Ok(())
}
