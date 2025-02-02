use echo::channel_service_client::ChannelServiceClient;
use echo::chat_service_client::ChatServiceClient;
use echo::{Channel, Message};
use std::collections::HashSet;
use std::str::FromStr;
use tokio::time::timeout;
use tokio::time::Duration;
use tonic::transport::Endpoint;
use tonic::{Request, Status, Streaming};
mod common;
use common::server::*;

#[tokio::test]
async fn test_chat() {
    env_logger::init();
    let (config, join_handle, tdb) = init_manager_server(50054).await;
    let addr = config.server.url_with(false);
    let conn = Endpoint::from_str(&addr).unwrap().connect().await.unwrap();
    let token = register_login("test", conn.clone()).await;
    let mut chan_client = ChannelServiceClient::new(conn.clone());

    // create 1 channels
    let mut channels = Vec::new();
    for i in 0..1 {
        let channel = Channel {
            name: format!("channel_{}", i),
            ..Default::default()
        };

        let rsp = chan_client
            .create(intercept_token(Request::new(channel.clone()), &token))
            .await
            .unwrap()
            .into_inner();
        channels.push(rsp);
    }

    // 1. add 1 servers, use localhost:port to mock report
    let mut handles = Vec::new();
    let mut servers_addr = HashSet::new();
    for i in 1..2 {
        let port = 50054 + i;
        let (config, handle) = init_chat_server(port, &tdb, &addr).await;
        handles.push(handle);
        servers_addr.insert(config.server.url_with(false));
    }
    // 2. register and login 4 users
    let mut tokens = vec![];
    for i in 1..5 {
        let token = register_login(&format!("test_{}", i), conn.clone()).await;
        tokens.push(token);
    }

    // 2. try listen all channels

    for channel in channels {
        let (mut senders, mut receivers) = (vec![], vec![]);
        // a. users create connections to chat server
        for token in tokens.iter() {
            let req_chan = channel.clone();
            let rsp = chan_client
                .listen(intercept_token(Request::new(req_chan.clone()), token))
                .await
                .unwrap()
                .into_inner();

            let chat_token = rsp.token;
            let chat_addr = rsp.server.unwrap().addr;
            let chat_conn = Endpoint::from_str(&chat_addr)
                .unwrap()
                .connect()
                .await
                .unwrap();
            let mut chat_client = ChatServiceClient::new(chat_conn);

            let (tx, rx) = tokio::sync::mpsc::channel(10);
            senders.push(tx);
            let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
            let req = intercept_token(Request::new(stream), &chat_token);

            let inbound = chat_client.conn(req).await.unwrap().into_inner();
            receivers.push(inbound);
        }
        // b. choose one send messages, check others' recv()
        let expected = vec![
            Message {
                data: "hello".into(),
                ..Default::default()
            },
            Message {
                data: "world".into(),
                ..Default::default()
            },
            Message {
                data: "hello world".into(),
                ..Default::default()
            },
        ];
        let sender = senders[0].clone();
        let timeout_duration = Duration::from_secs(10);
        let messages = expected.clone();
        tokio::spawn(async move {
            for msg in messages {
                sender.send(msg).await.unwrap();
            }
        });
        for stream in receivers.iter_mut() {
            check_inbound(stream, &expected, timeout_duration)
                .await
                .unwrap();
        }
    }

    for handle in handles {
        handle.abort();
    }
    join_handle.abort();
    drop(tdb);
}

async fn check_inbound(
    stream: &mut Streaming<Message>,
    expected: &[Message],
    timeout_duration: Duration,
) -> Result<(), Status> {
    let mut count = 0;

    while count < expected.len() {
        match timeout(timeout_duration, stream.message()).await {
            Ok(Ok(Some(msg))) => {
                if msg.data != expected[count].data {
                    return Err(Status::internal(format!(
                        "Message mismatch at index {}: expected {:?}, got {:?}",
                        count, expected[count], msg
                    )));
                }
                count += 1;
            }
            Ok(Ok(None)) => {
                return Err(Status::internal(
                    "Stream closed before receiving all expected messages",
                ));
            }
            Ok(Err(e)) => {
                return Err(e);
            }
            Err(_) => {
                return Err(Status::deadline_exceeded(format!(
                    "Timed out waiting for message at index {}",
                    count
                )));
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_chat_disconnect() {
    env_logger::init();
    let (config, join_handle, tdb) = init_manager_server(50054).await;
    let addr = config.server.url_with(false);
    let conn = Endpoint::from_str(&addr).unwrap().connect().await.unwrap();
    let token = register_login("test", conn.clone()).await;
    let mut chan_client = ChannelServiceClient::new(conn.clone());

    // create 1 channels
    let mut channels = Vec::new();
    for i in 0..1 {
        let channel = Channel {
            name: format!("channel_{}", i),
            ..Default::default()
        };

        let rsp = chan_client
            .create(intercept_token(Request::new(channel.clone()), &token))
            .await
            .unwrap()
            .into_inner();
        channels.push(rsp);
    }

    // 1. add 1 servers, use localhost:port to mock report
    let mut handles = Vec::new();
    let mut servers_addr = HashSet::new();
    for i in 1..2 {
        let port = 50054 + i;
        let (config, handle) = init_chat_server(port, &tdb, &addr).await;
        handles.push(handle);
        servers_addr.insert(config.server.url_with(false));
    }
    // 2. register and login 1 users
    let mut tokens = vec![];
    for i in 1..2 {
        let token = register_login(&format!("test_{}", i), conn.clone()).await;
        tokens.push(token);
    }

    // 2. try listen all channels
    for channel in channels {
        // a. users create connections to chat server
        for token in tokens.iter() {
            let req_chan = channel.clone();
            let rsp = chan_client
                .listen(intercept_token(Request::new(req_chan.clone()), token))
                .await
                .unwrap()
                .into_inner();

            let chat_token = rsp.token;
            let chat_addr = rsp.server.unwrap().addr;
            let chat_conn = Endpoint::from_str(&chat_addr)
                .unwrap()
                .connect()
                .await
                .unwrap();
            let mut chat_client = ChatServiceClient::new(chat_conn);

            let (tx, rx) = tokio::sync::mpsc::channel(10);
            let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
            let req = intercept_token(Request::new(stream), &chat_token);

            let mut inbound = chat_client.conn(req).await.unwrap().into_inner();

            // close tx, and check inbound
            drop(tx);
            let msg = inbound.message().await.unwrap();
            assert!(msg.is_none());
        }
    }

    for handle in handles {
        handle.abort();
    }
    join_handle.abort();
    drop(tdb);
}

#[tokio::test]
async fn test_conn_wrong_server() {
    env_logger::init();
    let (config, join_handle, tdb) = init_manager_server(50054).await;
    let addr = config.server.url_with(false);
    let conn = Endpoint::from_str(&addr).unwrap().connect().await.unwrap();
    let token = register_login("test", conn.clone()).await;
    let mut chan_client = ChannelServiceClient::new(conn.clone());

    // create 20 channels
    let mut channels = Vec::new();
    for i in 0..20 {
        let channel = Channel {
            name: format!("channel_{}", i),
            ..Default::default()
        };

        let rsp = chan_client
            .create(intercept_token(Request::new(channel.clone()), &token))
            .await
            .unwrap()
            .into_inner();
        channels.push(rsp);
    }

    // 1. add 2 servers, use localhost:port to mock report
    let mut handles = Vec::new();
    let mut servers_addr = HashSet::new();
    for i in 1..3 {
        let port = 50054 + i;
        let (config, handle) = init_chat_server(port, &tdb, &addr).await;
        handles.push(handle);
        servers_addr.insert(config.server.url_with(false));
    }
    // 2. register and login 1 users
    let mut tokens = vec![];
    for i in 1..2 {
        let token = register_login(&format!("test_{}", i), conn.clone()).await;
        tokens.push(token);
    }

    // 2. try listen all channels

    for channel in channels {
        let (mut senders, mut receivers) = (vec![], vec![]);
        // a. users create connections to chat server
        for token in tokens.iter() {
            let req_chan = channel.clone();
            let rsp = chan_client
                .listen(intercept_token(Request::new(req_chan.clone()), token))
                .await
                .unwrap()
                .into_inner();

            let chat_token = rsp.token;
            let chat_addr = rsp.server.unwrap().addr;
            let chat_conn = Endpoint::from_str(&chat_addr)
                .unwrap()
                .connect()
                .await
                .unwrap();
            let mut chat_client = ChatServiceClient::new(chat_conn);

            let (tx, rx) = tokio::sync::mpsc::channel(10);
            senders.push(tx);
            let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
            let req = intercept_token(Request::new(stream), &chat_token);

            let inbound = chat_client.conn(req).await.unwrap().into_inner();
            receivers.push(inbound);
        }
        // b. choose one send messages, check others' recv()
        let expected = vec![
            Message {
                data: "hello".into(),
                ..Default::default()
            },
            Message {
                data: "world".into(),
                ..Default::default()
            },
            Message {
                data: "hello world".into(),
                ..Default::default()
            },
        ];
        let sender = senders[0].clone();
        let timeout_duration = Duration::from_secs(10);
        let messages = expected.clone();
        tokio::spawn(async move {
            for msg in messages {
                sender.send(msg).await.unwrap();
            }
        });
        for stream in receivers.iter_mut() {
            check_inbound(stream, &expected, timeout_duration)
                .await
                .unwrap();
        }
    }

    for handle in handles {
        handle.abort();
    }
    join_handle.abort();
    drop(tdb);
}

#[tokio::test]
async fn test_report() {
    env_logger::init();
    let (config, join_handle, tdb) = init_manager_server(50054).await;
    let addr = config.server.url_with(false);
    let conn = Endpoint::from_str(&addr).unwrap().connect().await.unwrap();
    let token = register_login("test", conn.clone()).await;
    let mut chan_client = ChannelServiceClient::new(conn.clone());

    // create 20 channels
    let mut channels = Vec::new();
    for i in 0..20 {
        let channel = Channel {
            name: format!("channel_{}", i),
            ..Default::default()
        };

        let rsp = chan_client
            .create(intercept_token(Request::new(channel.clone()), &token))
            .await
            .unwrap()
            .into_inner();
        channels.push(rsp);
    }

    // 1. add 2 servers, use localhost:port to mock report
    let mut handles = Vec::new();
    let mut servers_addr = HashSet::new();
    for i in 1..3 {
        let port = 50054 + i;
        let (config, handle) = init_chat_server(port, &tdb, &addr).await;
        handles.push(handle);
        servers_addr.insert(config.server.url_with(false));
    }
    // 2. register and login 1 users
    let mut tokens = vec![];
    for i in 1..2 {
        let token = register_login(&format!("test_{}", i), conn.clone()).await;
        tokens.push(token);
    }

    // 3. try listen two channels at same time
    let rsp = chan_client
        .listen(intercept_token(
            Request::new(channels[0].clone()),
            &tokens[0],
        ))
        .await;

    println!("rsp: {:?}", rsp);
    assert!(rsp.is_ok());
    let rsp = chan_client
        .listen(intercept_token(
            Request::new(channels[0].clone()),
            &tokens[0],
        ))
        .await;
    assert!(rsp.is_err());

    for handle in handles {
        handle.abort();
    }
    join_handle.abort();
    drop(tdb);
}
