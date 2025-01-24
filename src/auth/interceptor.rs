use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use tonic::{Request, Status};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

#[derive(Clone)]
pub struct MyInterceptor {
    secret: String,
}

impl MyInterceptor {
    pub fn new() -> Self {
        Self {
            secret: "secret".to_string(),
        }
    }

    fn extract(&self, token: &str) -> Result<Claims, tonic::Status> {
        let decoding_key = DecodingKey::from_secret(self.secret.as_ref());
        let validation = Validation::new(Algorithm::HS256);
        decode::<Claims>(token, &decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|_| tonic::Status::unauthenticated("Invalid token"))
    }

    pub fn encrypt(&self, claims: &Claims) -> String {
        let encoding_key = EncodingKey::from_secret(self.secret.as_ref());
        encode(&Header::default(), claims, &encoding_key).unwrap()
    }
}

impl Default for MyInterceptor {
    fn default() -> Self {
        Self::new()
    }
}

impl tonic::service::Interceptor for MyInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let authorization = request
            .metadata()
            .get("authorization")
            .ok_or_else(|| tonic::Status::unauthenticated("No auth token provided"))?
            .to_str()
            .map_err(|e| tonic::Status::unauthenticated(e.to_string()))?;
        let token = &authorization["Bearer ".len()..];

        let claims = self.extract(token)?;
        request
            .metadata_mut()
            .insert("user_id", claims.sub.parse().unwrap());
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::service::Interceptor;
    use tonic::Request;

    #[test]
    fn test_extraction_and_encryption() {
        let interceptor = MyInterceptor::new();

        let claims = Claims {
            sub: "1234567890".to_string(),
            exp: 10000000000,
        };
        let token = interceptor.encrypt(&claims);

        let extracted_claims = interceptor.extract(&token).unwrap();
        assert_eq!(extracted_claims.sub, "1234567890");
    }

    #[test]
    fn test_interceptor() {
        let mut interceptor = MyInterceptor::new();

        let claims = Claims {
            sub: "1234567890".to_string(),
            exp: 10000000000,
        };
        let token = interceptor.encrypt(&claims);

        let mut request = Request::new(());
        request.metadata_mut().insert(
            "authorization",
            format!("Bearer {}", token).parse().unwrap(),
        );

        let result = interceptor.call(request);

        assert!(result.is_ok());
    }
}
