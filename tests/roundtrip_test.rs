use std::error::Error;

use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, service::service_fn, Response};
use hyper_named_pipe::{NamedPipeConnector, NAMED_PIPE_SCHEME};
use hyper_util::{
    client::legacy::Client,
    rt::{TokioExecutor, TokioIo},
};
use tokio::net::windows::named_pipe::ServerOptions;

#[tokio::test]
async fn test_roundtrip() -> Result<(), Box<dyn Error + Send + Sync>> {
    let pipe_name = r"\\.\pipe\named-pipe-test-server";
    let phrase = "hyper-named-pipe";

    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(pipe_name)?;

    let service =
        service_fn(|_req| async { Ok::<_, hyper::Error>(Response::new(phrase.to_string())) });

    let _server = tokio::spawn(async move {
        server
            .connect()
            .await
            .expect("Failed to connect to named pipe");

        let io = TokioIo::new(server);

        hyper::server::conn::http1::Builder::new()
            .serve_connection(io, service)
            .await
            .expect("Failed to wrap named pipe server with hyper HTTP interface")
    });

    let builder = Client::builder(TokioExecutor::new());
    let client: Client<NamedPipeConnector, Full<Bytes>> = builder.build(NamedPipeConnector);

    let host = hex::encode(pipe_name);
    let uri_str = format!("{}://{}", NAMED_PIPE_SCHEME, host);
    let url = uri_str.parse().expect("Failed to parse uri");

    let mut response = client
        .get(url)
        .await
        .expect("Failed to get URL using hyper client");
    let mut bytes = Vec::default();

    while let Some(frame_result) = response.frame().await {
        let frame = frame_result.expect("Failed to fetch chunk from HTTP response");

        if let Some(segment) = frame.data_ref() {
            bytes.extend(segment.iter().as_slice());
        }
    }

    let string = String::from_utf8(bytes).expect("Failed to utf8 encode HTTP response bytes");

    assert_eq!(phrase, string);

    Ok(())
}
