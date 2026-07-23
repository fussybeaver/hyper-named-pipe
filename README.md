[![crates.io](https://img.shields.io/crates/v/hyper-named-pipe.svg)](https://crates.io/crates/hyper-named-pipe)
[![license](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/license/apache-2.0)
[![AppVeyor](https://ci.appveyor.com/api/projects/status/7ogj03ayge78cl2t/branch/master?svg=true)](https://ci.appveyor.com/project/fussybeaver/hyper-named-pipe)
[![docs](https://docs.rs/hyper-named-pipe/badge.svg)](https://docs.rs/hyper-named-pipe)

## Hyper-named-pipe: Hyper client bindings for Windows Named Pipes

Exposes a HTTP interface over Tokio's Named Pipes implementation through a Hyper Connection
interface.

## Install

Add the following to your `Cargo.toml` file

```nocompile
[dependencies]
hyper-named-pipe = "*"
```

## Usage

Ensure host's are hex-encoded when passed into `HyperUri` as this will bypass validation.

```rust
let pipe_name = r"\\.\pipe\named-pipe";

let builder = Client::builder(TokioExecutor::new());
let client: Client<NamedPipeConnector, Full<Bytes>> = builder.build(NamedPipeConnector);

let host = hex::encode(pipe_name);
let uri_str = format!("{}://{}", NAMED_PIPE_SCHEME, host);
let url = uri_str.parse().expect("Invalid URI");

async move {
  client.get(url).await.expect("Unable to fetch URL with Hyper client");
};
```

## Tests

It's possible to run the unit test on a unix platform using the [`cross`](https://github.com/cross-rs/cross) helper:

```bash
cross test --target i686-pc-windows-gnu
```


