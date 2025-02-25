pub struct Config {
    pub server: ServerConfig,
    pub opensearch: OpenSearchConfig,
}

pub struct ServerConfig {
    pub port: u16,
    pub http_port: u16,
}

pub struct OpenSearchConfig {
    pub host: String,
    pub username: String,
    pub password: String,
    pub tls_insecure_skip_verify: bool,

    pub index_name: String,
    pub index_with_date_suffix: bool,
}
