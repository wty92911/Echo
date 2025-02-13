use std::collections::HashMap;

pub struct UserConfig {
    pub other_volume: HashMap<String, u8>, // percent
    pub input_volume: u8,
    pub output_volume: u8,
}

impl UserConfig {
    pub fn new(input_volume: u8, output_volume: u8) -> Self {
        Self {
            other_volume: HashMap::new(),
            input_volume,
            output_volume,
        }
    }
}

impl Default for UserConfig {
    fn default() -> Self {
        Self::new(100, 100)
    }
}
