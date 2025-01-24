use echo::{channel_service_client::ChannelServiceClient, user_service_client::UserServiceClient};
use echo::{Channel, LoginRequest, RegisterRequest};
use std::str::FromStr;
use tonic::transport::Endpoint;
use tonic::Request;
mod common;
use common::server::{create_interceptor, init};

#[tokio::test]
async fn test_channels() {
    let (config, join_handle, tdb) = init(50051).await;
    let addr = config.server.url_with(false);
    let conn = Endpoint::from_str(&addr).unwrap().connect().await.unwrap();

    let mut client = UserServiceClient::new(conn.clone());

    client
        .register(RegisterRequest {
            user_id: "test".to_string(),
            password: "test_password".to_string(),
            name: "test_name".to_string(),
        })
        .await
        .unwrap();

    let token = client
        .login(LoginRequest {
            user_id: "test".to_string(),
            password: "test_password".to_string(),
        })
        .await
        .unwrap()
        .into_inner()
        .token;

    let mut chan_client =
        ChannelServiceClient::with_interceptor(conn.clone(), create_interceptor(token));

    let channel1 = chan_client
        .create(Request::new(Channel {
            id: 0,
            name: "test1".to_string(),
            users: vec![],
            limit: 5,
        }))
        .await
        .unwrap()
        .into_inner();

    let channel2 = chan_client
        .create(Request::new(Channel {
            id: 0,
            name: "test2".to_string(),
            users: vec![],
            limit: 6,
        }))
        .await
        .unwrap()
        .into_inner();
    //list single
    let mut req_chan = Channel {
        id: channel1.id,
        ..Channel::default()
    };
    let rsp = chan_client
        .list(Request::new(req_chan.clone()))
        .await
        .unwrap()
        .into_inner()
        .channels;
    assert_eq!(rsp.len(), 1);
    assert_eq!(rsp[0], channel1);

    req_chan.id = 0;
    let rsp = chan_client
        .list(Request::new(req_chan.clone()))
        .await
        .unwrap()
        .into_inner()
        .channels;
    assert_eq!(rsp.len(), 2);

    // try delete channel2
    req_chan.id = channel2.id;
    let rsp = chan_client.delete(Request::new(req_chan.clone())).await;
    assert!(rsp.is_ok());

    // list again, should be 1
    req_chan.id = 0;
    let rsp = chan_client
        .list(Request::new(req_chan.clone()))
        .await
        .unwrap()
        .into_inner()
        .channels;
    assert_eq!(rsp.len(), 1);
    assert_eq!(rsp[0], channel1);

    // use another user token
    client
        .register(RegisterRequest {
            user_id: "test2".to_string(),
            password: "test_password".to_string(),
            name: "test_name".to_string(),
        })
        .await
        .unwrap();

    let token = client
        .login(LoginRequest {
            user_id: "test2".to_string(),
            password: "test_password".to_string(),
        })
        .await
        .unwrap()
        .into_inner()
        .token;

    let mut chan_client =
        ChannelServiceClient::with_interceptor(conn.clone(), create_interceptor(token));

    // try delete channel1, fail
    req_chan.id = channel1.id;
    let rsp = chan_client.delete(Request::new(req_chan.clone())).await;
    assert!(rsp.is_err());

    join_handle.abort();
    drop(tdb);
}
