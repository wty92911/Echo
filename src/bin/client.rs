use std::thread::sleep;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use tonic::codegen::tokio_stream::wrappers::UnboundedReceiverStream;
use echo::service::echo::{service_client::ServiceClient, ChatRequest, Message, LoginRequest, User, ListenRequest};

#[tokio::main]
async fn main() {

    let mut client = ServiceClient::connect("http://[::1]:7878").await.unwrap();
    
    
    let request = tonic::Request::new(
        LoginRequest {
            user: Some(User {
                id: 1,
                name: "test".to_string(),
            })
        }

    );
    let response = client.login(request).await.unwrap();
    println!("RESPONSE={:?}", response);
    let token = response.into_inner().token.clone();
    let token_clone = token.clone();
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        for i in 0..100 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let request = ChatRequest {
                token: token_clone.clone(),
                channel: 1,
                message: Some(Message {
                    user_id: 1,
                    time_stamp: i.into(),
                    order: i,
                    message: format!("Hello, world {}!", i)
                }),
            };
            tx.send(request).unwrap();
        }
    });
    let stream = UnboundedReceiverStream::new(rx);
    let response = client.chat(stream).await.unwrap();
    println!("RESPONSE={:?}", response);

    let req = tonic::Request::new(ListenRequest {
        token: token.clone(),
        channel: 1,
    });
    let response = client.listen(req).await.unwrap();
    let mut rsp = response.into_inner();
    while let Some(msg) = rsp.message().await.unwrap() {
        println!("Receiving MESSAGE={:?}", msg.message.unwrap());
    }
}

