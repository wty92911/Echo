use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tonic::{Request, Status};
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

/// insert token of `Claims` into request metadata
///
/// use `secret` to encrypt token
pub fn intercept_token<T>(
    mut request: Request<T>,
    claims: impl Serialize,
    secret: &str,
) -> Result<Request<T>, Status> {
    let token = encrypt(secret, &claims);
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", token).parse().unwrap(),
    );
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Request;

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

    #[tokio::test]
    async fn test_intercept_token() {
        let secret = "secret";
        let claims = Claims {
            sub: "123".to_string(),
            exp: 10000000000,
            user_id: "user_123".to_string(),
            channel_id: 1,
            ..Default::default()
        };

        // Create a dummy request
        let request = Request::new(());

        // Intercept the request with the token
        let intercepted_request = intercept_token(request, &claims, secret).unwrap();

        // Get the token from the intercepted request metadata
        let token = intercepted_request
            .metadata()
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .trim_start_matches("Bearer ");

        // Extract the claims from the token
        let extracted_claims: Claims = extract(secret, token).unwrap();

        // Assert that the extracted claims match the original claims
        assert_eq!(claims.sub, extracted_claims.sub);
        assert_eq!(claims.exp, extracted_claims.exp);
        assert_eq!(claims.user_id, extracted_claims.user_id);
        assert_eq!(claims.channel_id, extracted_claims.channel_id);
    }
}
