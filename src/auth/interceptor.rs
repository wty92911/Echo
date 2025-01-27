use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use tonic::{Request, Status};
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
}

/// Authentication interceptor on server
#[derive(Clone)]
pub struct AuthTokenInterceptor {
    secret: String,
}

fn extract(secret: &str, token: &str) -> Result<Claims, tonic::Status> {
    let decoding_key = DecodingKey::from_secret(secret.as_ref());
    let validation = Validation::new(Algorithm::HS256);
    decode::<Claims>(token, &decoding_key, &validation)
        .map(|data| data.claims)
        .map_err(|e| tonic::Status::unauthenticated(format!("Invalid token {}", e)))
}

pub fn encrypt(secret: &str, claims: &Claims) -> String {
    let encoding_key = EncodingKey::from_secret(secret.as_ref());
    encode(&Header::default(), claims, &encoding_key).unwrap()
}

// for auth
impl AuthTokenInterceptor {
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }
}

impl Default for AuthTokenInterceptor {
    fn default() -> Self {
        Self::new("secret")
    }
}

impl tonic::service::Interceptor for AuthTokenInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let authorization = request
            .metadata()
            .get("authorization")
            .ok_or_else(|| tonic::Status::unauthenticated("No auth token provided"))?
            .to_str()
            .map_err(|e| tonic::Status::unauthenticated(e.to_string()))?;
        let token = &authorization["Bearer ".len()..];

        let claims = extract(&self.secret, token)?;
        request
            .metadata_mut()
            .insert("token", claims.sub.parse().unwrap());
        Ok(request)
    }
}

/// Client token interceptor
pub struct ClientTokenInterceptor {
    secret: String,
    claims: Claims,
}

impl ClientTokenInterceptor {
    pub fn new(secret: impl Into<String>, claims: Claims) -> Self {
        Self {
            secret: secret.into(),
            claims,
        }
    }

    pub fn call<T>(&self, mut request: Request<T>) -> Result<Request<T>, Status> {
        let token = encrypt(&self.secret, &self.claims);
        request.metadata_mut().insert(
            "authorization",
            format!("Bearer {}", token).parse().unwrap(),
        );
        Ok(request)
    }
}

// impl tonic::service::Interceptor for ClientTokenInterceptor {
//     fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
//         let token = encrypt(&self.secret, &self.claims);
//         request.metadata_mut().insert(
//             "authorization",
//             format!("Bearer {}", token).parse().unwrap(),
//         );
//         Ok(request)
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::service::Interceptor;
    use tonic::Request;

    #[test]
    fn test_extraction_and_encryption() {
        let interceptor = AuthTokenInterceptor::default();

        let claims = Claims {
            sub: "1234567890".to_string(),
            exp: 10000000000,
        };
        let token = encrypt(&interceptor.secret, &claims);

        let extracted_claims = extract(&interceptor.secret, &token).unwrap();
        assert_eq!(extracted_claims.sub, "1234567890");
    }

    #[test]
    fn test_server_interceptor() {
        let mut interceptor = AuthTokenInterceptor::default();

        let claims = Claims {
            sub: "1234567890".to_string(),
            exp: 10000000000,
        };
        let token = encrypt(&interceptor.secret, &claims);

        let mut request = Request::new(());
        request.metadata_mut().insert(
            "authorization",
            format!("Bearer {}", token).parse().unwrap(),
        );

        let result = interceptor.call(request);

        assert!(result.is_ok());
        let request = result.unwrap();
        let token = request.metadata().get("token").unwrap().to_str().unwrap();
        assert_eq!(token, "1234567890");
    }

    #[test]
    fn test_client_interceptor() {
        let claims = Claims {
            sub: "1234567890".to_string(),
            exp: 10000000000,
        };
        let interceptor = ClientTokenInterceptor::new("secret", claims);

        let request = Request::new(());
        let result = interceptor.call(request);

        assert!(result.is_ok());
        let request = result.unwrap();
        let authorization = request
            .metadata()
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(authorization.starts_with("Bearer "));
    }

    #[test]
    fn test_invalid_token_server_interceptor() {
        let mut interceptor = AuthTokenInterceptor::default();

        let mut request = Request::new(());
        request
            .metadata_mut()
            .insert("authorization", "Bearer invalid_token".parse().unwrap());

        let result = interceptor.call(request);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn test_missing_token_server_interceptor() {
        let mut interceptor = AuthTokenInterceptor::default();

        let request = Request::new(());

        let result = interceptor.call(request);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn test_token_expired_server_interceptor() {
        let mut interceptor = AuthTokenInterceptor::default();

        let mut request = Request::new(());

        let claims = Claims {
            sub: "1234567890".to_string(),
            exp: 1,
        };
        let client_interceptor = ClientTokenInterceptor::new("secret", claims);

        request = client_interceptor.call(request).unwrap();
        let result = interceptor.call(request);
        println!("result: {:?}", result);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }
}
