// Example Rust file for lfv testing
use std::collections::HashMap;

pub struct Config {
    pub api_key: String,
}

impl Config {
    pub fn new() -> Self {
        Self {
            api_key: "sk-test1234567890abcdef".to_string(),
        }
    }
}
