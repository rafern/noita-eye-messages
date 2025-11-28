#[derive(prost::Message)]
pub struct KeyDumpMeta {
    #[prost(string, tag = "1")]
    pub build_hash: String,
    #[prost(string, tag = "2")]
    pub cipher_name: String,
    #[prost(string, optional, tag = "3")]
    pub cipher_config: Option<String>,
}