use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,

    // other fields
    pub user_id: String,
    pub channel_id: i32,
    pub addr: String,
}

impl Default for Claims {
    fn default() -> Self {
        Claims {
            sub: "".to_string(),
            exp: 0,
            user_id: "".to_string(),
            channel_id: 0,
            addr: "".to_string(),
        }
    }
}

pub fn extract<T: DeserializeOwned>(secret: &str, token: &str) -> Result<T, tonic::Status> {
    let decoding_key = DecodingKey::from_secret(secret.as_ref());
    let validation = Validation::new(Algorithm::HS256);
    decode::<T>(token, &decoding_key, &validation)
        .map(|data| data.claims)
        .map_err(|e| tonic::Status::unauthenticated(format!("Invalid token {}", e)))
}

pub fn encrypt<T: Serialize>(secret: &str, claims: &T) -> String {
    let encoding_key = EncodingKey::from_secret(secret.as_ref());
    encode(&Header::default(), claims, &encoding_key).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extract_and_encrypt() {
        let secret = "secret";
        let claims = Claims {
            sub: "123".to_string(),
            exp: 10000000000,
            user_id: "user_123".to_string(),
            channel_id: 1,
            ..Default::default()
        };

        // Encrypt the claims to create a token
        let token = encrypt(secret, &claims);

        // Extract the claims from the token
        let extracted_claims: Claims = extract(secret, &token).unwrap();

        // Assert that the extracted claims match the original claims
        assert_eq!(claims.sub, extracted_claims.sub);
        assert_eq!(claims.exp, extracted_claims.exp);
        assert_eq!(claims.user_id, extracted_claims.user_id);
        assert_eq!(claims.channel_id, extracted_claims.channel_id);
    }
}
