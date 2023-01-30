use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub struct Configuration {
    pub discord_token: String,
    pub hord_path: String,
    pub expunged_message: String,
    pub admin_users: Vec<String>,
}

fn default_configuration() -> Configuration {
    Configuration {
        discord_token: "DISCORD-TOKEN-HERE".to_string(),
        hord_path: "~/.config/fyrnwita/hord.sl3".to_string(),
        expunged_message: "Quote has been removed.".to_string(),
        admin_users: vec![],
    }
}

pub fn load_configuration(path: &str) -> Configuration {
    let binding = shellexpand::tilde(path).to_string();
    let expanded_path = Path::new(&binding);
    if expanded_path.exists() {
        let data = fs::read_to_string(expanded_path).expect("Unable to read file");
        serde_json::from_str(&data).expect("Unable to parse JSON file")
    } else {
        let configuration = default_configuration();
        fs::write(
            expanded_path,
            serde_json::to_string_pretty(&configuration).unwrap(),
        )
        .expect("Unable to write to config file");
        configuration
    }
}
