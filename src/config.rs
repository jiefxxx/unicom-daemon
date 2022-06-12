#[derive(Debug, Deserialize)]
pub struct Config {
    pub unix_stream_path: String,
    pub server_addr: String,
    pub template_dir: String,
    pub app_dir: String,
    pub session_path: String,
}

pub fn read_config() -> Config{
    let content = std::fs::read_to_string("config.toml").unwrap();
    toml::from_str(&content).unwrap()
}