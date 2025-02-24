use std::error::Error;

use opensearch::{BulkParts, OpenSearch, http::request::JsonBody};
use opentelemetry_proto::tonic::{
    collector::logs::v1::{
        ExportLogsPartialSuccess, ExportLogsServiceRequest, ExportLogsServiceResponse,
    },
    common::v1::any_value::Value as OtelValue,
};

use crate::{core::error::ApplicationError, utils::chrono::format_iso8601};

use super::mapper::map_otel_value_to_serdejson_value;

#[derive(Debug, Default)]
pub struct Exporter {
    client: OpenSearch,
    index: String,
    index_append_date_suffix: bool,
}

impl Exporter {
    pub fn new(
        client: OpenSearch,
        index: String,
        index_append_date_suffix: bool,
    ) -> Self {
        Self { client, index, index_append_date_suffix }
    }
}

impl Exporter {
    pub async fn export(
        &self,
        request: ExportLogsServiceRequest,
    ) -> Result<ExportLogsServiceResponse, ApplicationError> {
        let bulk_index_template = serde_json::json!({"index": {}});
        let mut bulk_body: Vec<JsonBody<serde_json::Value>> = Vec::new();
        for resouce_log in request.resource_logs {
            for scope_log in resouce_log.scope_logs {
                for log_record in scope_log.log_records {
                    let mut log = serde_json::Map::new();
                    let mut attributes = serde_json::Map::new();
                    for attribute in log_record.attributes {
                        if let Some(v) = attribute.value {
                            attributes
                                .insert(attribute.key, map_otel_value_to_serdejson_value(v.value));
                        }
                    }
                    log.insert(
                        "attributes".to_owned(),
                        serde_json::Value::Object(attributes),
                    );

                    if let Some(b) = log_record.body {
                        if let Some(v) = b.value {
                            let b = match v {
                                OtelValue::StringValue(s) => {
                                    if let Ok(parsed) = serde_json::from_str(&s) {
                                        parsed
                                    } else {
                                        serde_json::Value::String(s)
                                    }
                                }
                                vv => map_otel_value_to_serdejson_value(Some(vv)),
                            };
                            log.insert("body".to_owned(), b);
                        }
                    }
                    log.insert(
                        "@timestamp".to_owned(),
                        serde_json::Value::String(
                            format_iso8601(chrono::DateTime::from_timestamp_nanos(log_record.time_unix_nano as i64))
                        ),
                    );
                    bulk_body.push(bulk_index_template.clone().into());
                    bulk_body.push(Into::<serde_json::Value>::into(log).into());
                }
            }
        }

        let index = if self.index_append_date_suffix {
            &format!("{}-{}", self.index, chrono::Local::now().format("%y%m%d"))
        } else {
            &self.index
        };
        
        let bulk_response = self
            .client
            .bulk(BulkParts::Index(index))
            .body(bulk_body)
            .send()
            .await;

        match bulk_response {
            Ok(resp) => {
                let resp_body: serde_json::Value = match resp.json().await {
                    Ok(resp_body) => resp_body,
                    Err(err) => {
                        // TODO: logging
                        return Err(ApplicationError::NonRetryable(err.into()));
                    }
                };

                match resp_body["errors"].as_bool() {
                    Some(true) => {
                        let Some(items) = resp_body["items"].as_array() else {
                            // TODO: logging
                            return Err(ApplicationError::NonRetryable(
                                "cannot find items in bulk error respose".into(),
                            ));
                        };

                        let mut error_count = 0;
                        for it in items {
                            let index = match it["index"].as_object() {
                                Some(v) => v,
                                None => {
                                    // TODO: logging
                                    error_count += 1;
                                    continue;
                                }
                            };
                            let status = match index["status"].as_i64() {
                                Some(v) => v,
                                None => {
                                    // TODO: logging
                                    error_count += 1;
                                    continue;
                                }
                            };

                            // TODO: do we need to check for 201.
                            if status != 200 || status != 201 {
                                error_count += 1;
                            }
                        }

                        return Ok(ExportLogsServiceResponse {
                            partial_success: Some(ExportLogsPartialSuccess {
                                rejected_log_records: error_count,
                                // TODO: optimize, maybe we should return only _id and reason for each error.
                                error_message: resp_body.to_string(),
                            }),
                        });
                    }
                    Some(false) => {
                        return Ok(ExportLogsServiceResponse {
                            ..Default::default()
                        });
                    }
                    None => {
                        // TODO: logging
                        return Ok(ExportLogsServiceResponse {
                            ..Default::default()
                        });
                    }
                }
            }
            Err(err) => {
                if let Some(err) = err.source() {
                    if let Some(ioerr) = err.downcast_ref::<std::io::Error>() {
                        // IMPORTANT: we should retry only on IO error
                        return Err(ApplicationError::Retryable(ioerr.to_string().into()));
                    }
                }
                return Err(ApplicationError::NonRetryable(err.to_string().into()));
            }
        }
    }
}
