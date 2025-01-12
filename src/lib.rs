mod core;
pub mod service;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

fn generate_token() -> String {
    let mut rng = thread_rng();
    (0..32).map(|_| rng.sample(Alphanumeric) as char).collect()
}
