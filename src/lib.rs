//! [![crates.io](https://img.shields.io/crates/v/hyper-named-pipe.svg)](https://crates.io/crates/bollard)
//! [![license](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
//! [![appveyor](https://ci.appveyor.com/api/projects/status/n5khebyfae0u1sbv/branch/master?svg=true)](https://ci.appveyor.com/project/fussybeaver/boondock)
//! [![docs](https://docs.rs/bollard/badge.svg)](https://docs.rs/hyper-named-pipe/)
//!
//! # Hyper-named-pipe: Hyper client bindings for Windows Named Pipes
//!
//! Exposes a HTTP interface over Tokio's Named Pipes implementation through a Hyper Connection
//! interface.
//!
//! # Install
//!
//! Add the following to your `Cargo.toml` file
//!
//! ```nocompile
//! [dependencies]
//! hyper-named-pipe = "*"
//! ```
//!
//! # Usage
//!
//! Please ensure hostname's are hex-encoded when passed into `http::Uri`, as we need to bypass
//! `http::Uri`'s restrictive validation that will not accept a standard named pipe.
//!
//! ```rust,no_run
//! # use http_body_util::{BodyExt, Full};
//! # use hyper::body::Bytes;
//! # use hyper_named_pipe::{NamedPipeConnector, NAMED_PIPE_SCHEME};
//! # use hyper_util::{
//! #     client::legacy::Client,
//! #     rt::{TokioExecutor, TokioIo},
//! # };
//! let pipe_name = r"\\.\pipe\named-pipe";
//!
//! let builder = Client::builder(TokioExecutor::new());
//! let client: Client<NamedPipeConnector, Full<Bytes>> = builder.build(NamedPipeConnector);
//!
//! let host = hex::encode(pipe_name);
//! let uri_str = format!("{}://{}", NAMED_PIPE_SCHEME, host);
//! let url = uri_str.parse().expect("Invalid URI");
//!
//! async move {
//!   client.get(url).await.expect("Unable to fetch URL with Hyper client");
//! };
//! ```
//!
//! # Tests
//!
//! It's possible to run the unit test on a unix platform using the [`cross`](https://github.com/cross-rs/cross) helper:
//!
//! ```bash
//! cross test --target i686-pc-windows-gnu
//! ```
//!
//!
#![deny(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]
#![warn(rust_2018_idioms)]

use hex::FromHex;
use hyper::rt::ReadBufCursor;
use hyper_util::client::legacy::connect::{Connected, Connection};
use hyper_util::rt::TokioIo;
use pin_project_lite::pin_project;
use tokio::io::AsyncWrite;
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
use tokio::time;

use std::ffi::OsStr;
use std::future::Future;
use std::io;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use winapi::shared::winerror;

/// The scheme part of a uri that denotes a named pipe connection
pub const NAMED_PIPE_SCHEME: &str = "net.pipe";

pin_project! {
    /// The Hyper plumbing to read/write to an underlying Tokio Named Pipe Client interface
    pub struct NamedPipeStream {
        #[pin]
        io: NamedPipeClient,
    }
}

impl NamedPipeStream {
    /// The internal connection implementation to initialize a Tokio Named Pipe Client interface
    pub async fn connect<A>(addr: A) -> Result<NamedPipeStream, io::Error>
    where
        A: AsRef<Path> + AsRef<OsStr>,
    {
        let opts = ClientOptions::new();

        let client = loop {
            match opts.open(&addr) {
                Ok(client) => break client,
                Err(e) if e.raw_os_error() == Some(winerror::ERROR_PIPE_BUSY as i32) => (),
                Err(e) => return Err(e),
            };

            time::sleep(Duration::from_millis(50)).await;
        };

        Ok(NamedPipeStream { io: client })
    }
}

impl hyper::rt::Read for NamedPipeStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        let mut t = TokioIo::new(self.project().io);
        Pin::new(&mut t).poll_read(cx, buf)
    }
}

impl hyper::rt::Write for NamedPipeStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write_vectored(cx, bufs)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

#[derive(Clone, Copy, Debug)]
/// The end-user interface to initialize a Hyper Client
///
/// # Example
///
/// ```rust
/// # use http_body_util::{BodyExt, Full};
/// # use hyper::body::Bytes;
/// # use hyper_named_pipe::NamedPipeConnector;
/// # use hyper_util::{
/// #     client::legacy::Client,
/// #     rt::{TokioExecutor, TokioIo},
/// # };
/// let builder = Client::builder(TokioExecutor::new());
/// let client: Client<NamedPipeConnector, Full<Bytes>> = builder.build(NamedPipeConnector);
/// ```
pub struct NamedPipeConnector;

impl tower_service::Service<hyper::Uri> for NamedPipeConnector {
    type Response = NamedPipeStream;
    type Error = io::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, destination: hyper::Uri) -> Self::Future {
        let fut = async move {
            match destination.scheme() {
                Some(scheme) if scheme == NAMED_PIPE_SCHEME => Ok(()),
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Invalid scheme {:?}", destination.scheme()),
                )),
            }?;

            if let Some(host) = destination.host() {
                let bytes = Vec::from_hex(host).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid URL, host must be a hex-encoded path",
                    )
                })?;

                Ok(NamedPipeStream::connect(String::from_utf8_lossy(&bytes).into_owned()).await?)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Invalid uri {:?}", destination),
                ))
            }
        };

        Box::pin(fut)
    }
}

impl Connection for NamedPipeStream {
    fn connected(&self) -> Connected {
        Connected::new()
    }
}
