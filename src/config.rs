use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmojiTypes {
    Emoji(char),
    CustomEmoji(String),
}

#[derive(Serialize, Deserialize)]
pub struct Configuration {
    pub hord_path: String,
    pub expunged_message: String,
    pub admin_users: Vec<String>,
    pub reactions: HashMap<String, EmojiTypes>,
}

fn default_configuration() -> Configuration {
    Configuration {
        hord_path: "/var/lib/fyrnwita/hord.sl3".to_string(),
        expunged_message: "Quote has been removed.".to_string(),
        admin_users: vec![],
        reactions: HashMap::new(),
    }
}

pub fn load_configuration(path: &str) -> Configuration {
    let path = Path::new(&path);
    if path.exists() {
        let data = fs::read_to_string(path).expect("Unable to read file");
        serde_json::from_str(&data).expect("Unable to parse JSON file")
    } else {
        let configuration = default_configuration();
        fs::write(path, serde_json::to_string_pretty(&configuration).unwrap())
            .expect("Unable to write to config file");
        configuration
    }
}
