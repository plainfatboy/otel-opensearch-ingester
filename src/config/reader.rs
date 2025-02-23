use std::env;

use crate::core::error::BoxDynError;

use super::{Config, OpenSearchConfig, ServerConfig};

fn read_required_env(key: &str) -> Result<String, BoxDynError> {
    env::var(key).map_err(|e| match e {
        env::VarError::NotPresent => format!("{} is not set", key).into(),
        env::VarError::NotUnicode(_) => format!("{} is not valid unicode", key).into(),
    })
}

pub fn read_config_from_env() -> Result<Config, BoxDynError> {
    let server = ServerConfig {
        port: match env::var("PORT") {
            Ok(val) => val.parse()?,
            Err(err) => match err {
                env::VarError::NotPresent => 4317,
                env::VarError::NotUnicode(_) => return Err("PORT is not valid unicode".into()),
            }
        }
    };

    let opensearch = OpenSearchConfig {
        host: read_required_env("OPENSEARCH_HOST")?,
        username: read_required_env("OPENSEARCH_USERNAME")?,
        password: read_required_env("OPENSEARCH_PASSWORD")?,
        tls_insecure_skip_verify: read_required_env("OPENSEARCH_TLS_INSECURE_SKIP_VERIFY")?.parse()?,
        index_name: read_required_env("OPENSEARCH_INDEX_NAME")?,
        index_with_date_suffix: match env::var("OPENSEARCH_INDEX_WITH_DATE_SUFFIX") {
            Ok(val) => val.parse()?,
            Err(err)  => match err {
                env::VarError::NotPresent => true,
                env::VarError::NotUnicode(_) => return Err("OPENSEARCH_INDEX_WITH_DATE_SUFFIX is not valid unicode".into()),
            }
        }
    };
    Ok(Config { server, opensearch })
}