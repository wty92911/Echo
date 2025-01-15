use echo::{
    service_client::ServiceClient, user_service_client::UserServiceClient, ChatRequest,
    ListenRequest, ListenResponse, LoginRequest, Message, User,
};
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tonic::codegen::tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Status};

pub struct AuthInterceptor {
    token: String,
}

impl tonic::service::Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        if !self.token.is_empty() {
            request.metadata_mut().insert(
                "authorization",
                format!("Bearer {}", self.token).parse().unwrap(),
            );
        }
        Ok(request)
    }
}

pub struct Client {
    service_cli: ServiceClient<InterceptedService<Channel, AuthInterceptor>>,
    user_service_cli: UserServiceClient<Channel>,
}

impl Client {
    pub async fn new(user: User) -> echo::Result<Self> {
        let endpoint = Endpoint::from_static("http://[::1]:7878");
        let channel = endpoint.connect().await?;

        let mut user_service_cli = UserServiceClient::new(channel.clone());

        let request = tonic::Request::new(LoginRequest { user: Some(user) });

        let token = user_service_cli
            .login(request)
            .await?
            .into_inner()
            .token
            .clone();

        let service_cli =
            ServiceClient::with_interceptor(channel.clone(), AuthInterceptor { token });

        Ok(Self {
            service_cli,
            user_service_cli,
        })
    }

    pub async fn login(&mut self, user: User) -> echo::Result<String> {
        let request = tonic::Request::new(LoginRequest { user: Some(user) });
        let response = self.user_service_cli.login(request).await?;
        Ok(response.into_inner().token)
    }

    pub async fn chat(
        &mut self,
        stream: impl tonic::IntoStreamingRequest<Message = ChatRequest>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.service_cli.chat(stream).await?;
        Ok(())
    }

    pub async fn listen(
        &mut self,
        request: ListenRequest,
    ) -> Result<UnboundedReceiver<ListenResponse>, Box<dyn std::error::Error>> {
        let mut response = self.service_cli.listen(request).await?.into_inner();

        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Some(msg) = response.message().await.unwrap() {
                tx.send(msg).unwrap();
            }
        });
        Ok(rx)
    }
}

#[tokio::main]
async fn main() -> echo::Result<()> {
    let mut client = Client::new(User {
        id: 1,
        name: "test".to_string(),
    })
    .await?;

    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        for i in 0..100 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let request = ChatRequest {
                channel: 1,
                message: Some(Message {
                    user_id: 1,
                    time_stamp: i.into(),
                    order: i,
                    message: format!("Hello, world {}!", i),
                }),
            };
            tx.send(request).unwrap();
        }
    });

    let stream = UnboundedReceiverStream::new(rx);

    client.chat(stream).await?;

    let listen_req = ListenRequest { channel: 1 };

    let mut rx = client.listen(listen_req).await?;

    while let Some(msg) = rx.recv().await {
        println!("Received message: {:?}", msg);
    }

    Ok(())
}
