use abi::pb::channel_service_client::ChannelServiceClient;
use abi::pb::Channel;
use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;
use tonic::transport::Endpoint;
use tonic::Request;
mod common;
use abi::traits::WithToken;
use common::server::*;

// async fn create_channels()

#[tokio::test]
async fn test_channels() {
    let (config, join_handle, tdb) = init_manager_server(50053).await;
    let addr = config.server.url_with(false);
    let conn = Endpoint::from_str(&addr).unwrap().connect().await.unwrap();

    let token = register_login("test", conn.clone()).await;

    let mut chan_client = ChannelServiceClient::new(conn.clone());

    let channel1 = chan_client
        .create(
            Request::new(Channel {
                id: 0,
                name: "test1".to_string(),
                users: vec![],
                limit: 5,
            })
            .with(&token),
        )
        .await
        .unwrap()
        .into_inner();

    let channel2 = chan_client
        .create(
            Request::new(Channel {
                id: 0,
                name: "test2".to_string(),
                users: vec![],
                limit: 6,
            })
            .with(&token),
        )
        .await
        .unwrap()
        .into_inner();
    //list single
    let mut req_chan = Channel {
        id: channel1.id,
        ..Channel::default()
    };
    let rsp = chan_client
        .list(Request::new(req_chan.clone()).with(&token))
        .await
        .unwrap()
        .into_inner()
        .channels;
    assert_eq!(rsp.len(), 1);
    assert_eq!(rsp[0], channel1);

    req_chan.id = 0;
    let rsp = chan_client
        .list(Request::new(req_chan.clone()).with(&token))
        .await
        .unwrap()
        .into_inner()
        .channels;
    assert_eq!(rsp.len(), 2);

    // try delete channel2
    req_chan.id = channel2.id;
    let rsp = chan_client
        .delete(Request::new(req_chan.clone()).with(&token))
        .await;
    assert!(rsp.is_ok());

    // list again, should be 1
    req_chan.id = 0;
    let rsp = chan_client
        .list(Request::new(req_chan.clone()).with(&token))
        .await
        .unwrap()
        .into_inner()
        .channels;
    assert_eq!(rsp.len(), 1);
    assert_eq!(rsp[0], channel1);

    // use another user token
    let token = "test2".to_string();

    // try delete channel1, fail
    req_chan.id = channel1.id;
    let rsp = chan_client
        .delete(Request::new(req_chan.clone()).with(&token))
        .await;
    assert!(rsp.is_err());

    join_handle.abort();
    drop(tdb);
}

// test listen to some channel, and add/del servers.
#[tokio::test]
async fn test_listen() {
    let (config, join_handle, tdb) = init_manager_server(50054).await;
    let addr = config.server.url_with(false);
    let conn = Endpoint::from_str(&addr).unwrap().connect().await.unwrap();
    let token = register_login("test", conn.clone()).await;

    let mut chan_client = ChannelServiceClient::new(conn.clone());

    // create 5 channels
    let mut channels = Vec::new();
    for i in 0..3 {
        let channel = Channel {
            name: format!("channel_{}", i),
            ..Default::default()
        };

        let rsp = chan_client
            .create(Request::new(channel.clone()).with(&token))
            .await
            .unwrap()
            .into_inner();
        channels.push(rsp);
    }

    // 1. try listen, expect server no found
    let req_chan = channels[0].clone();
    let rsp = chan_client
        .listen(Request::new(req_chan.clone()).with(&token))
        .await;
    assert!(rsp.is_err());
    println!("listen failed: {:?}", rsp.unwrap_err());

    // 2. add 3 servers, use localhost:port to mock report
    let mut handles = Vec::new();
    let mut servers_addr = HashSet::new();
    for i in 1..4 {
        let port = 50054 + i;
        let (config, handle) = init_chat_server(port, &tdb, &addr).await;
        handles.push(handle);
        servers_addr.insert(config.server.url_with(false));
    }

    // 3. try listen, expect server in above 3 servers
    for channel in channels {
        let req_chan = channel.clone();
        let rsp = chan_client
            .listen(Request::new(req_chan.clone()).with(&token))
            .await
            .unwrap()
            .into_inner();
        tokio::time::sleep(Duration::from_secs(1)).await;
        println!("listen success: {:?}", rsp);
        assert!(servers_addr.contains(&rsp.server.unwrap().addr));
    }
    // assert!(servers_addr.contains(&rsp.server.unwrap().addr));
    for handle in handles {
        handle.abort();
    }
    join_handle.abort();
    drop(tdb);
}
