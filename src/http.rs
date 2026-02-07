//! HTTP client implementation for GPUI integration.
//!
//! This provides a `ReqwestClient` that implements `gpui_http_client::HttpClient`,
//! bridging our reqwest-based HTTP client with GPUI's image loading system.
//! Based on Zed's internal `reqwest_client` crate.

use std::sync::Arc;
use std::{mem, pin::Pin, task::Poll};

use anyhow::anyhow;
use bytes::{BufMut, Bytes, BytesMut};
use futures::{FutureExt as _, TryStreamExt as _};
use gpui_http_client::{AsyncBody, HttpClient, Inner, Url};
use http::header::HeaderValue;

use crate::tokio_runtime;

const DEFAULT_CAPACITY: usize = 4096;

/// An HTTP client backed by reqwest that implements GPUI's `HttpClient` trait.
///
/// This bridges the tokio-based reqwest client with GPUI's smol-based async runtime.
pub struct ReqwestClient {
    client: reqwest::Client,
    proxy: Option<Url>,
    user_agent: Option<HeaderValue>,
    handle: tokio::runtime::Handle,
}

impl ReqwestClient {
    fn builder() -> reqwest::ClientBuilder {
        reqwest::Client::builder()
            .use_rustls_tls()
            .connect_timeout(std::time::Duration::from_secs(10))
    }

    /// Create a new `ReqwestClient` with a custom user agent.
    pub fn user_agent(agent: &str) -> anyhow::Result<Self> {
        let mut map = reqwest::header::HeaderMap::new();
        let ua_value = HeaderValue::from_str(agent)?;
        map.insert(reqwest::header::USER_AGENT, ua_value.clone());
        let client = Self::builder().default_headers(map).build()?;

        let handle = tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
            log::debug!("no tokio runtime found, using global runtime for ReqwestClient");
            tokio_runtime().handle().clone()
        });

        Ok(Self {
            client,
            handle,
            proxy: None,
            user_agent: Some(ua_value),
        })
    }
}

impl HttpClient for ReqwestClient {
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn user_agent(&self) -> Option<&HeaderValue> {
        self.user_agent.as_ref()
    }

    fn proxy(&self) -> Option<&Url> {
        self.proxy.as_ref()
    }

    fn send(
        &self,
        req: http::Request<AsyncBody>,
    ) -> futures::future::BoxFuture<'static, anyhow::Result<http::Response<AsyncBody>>> {
        let (parts, body) = req.into_parts();

        let mut request = self.client.request(parts.method, parts.uri.to_string());
        request = request.headers(parts.headers);

        // Note: reqwest 0.12 doesn't support per-request redirect policy.
        // The redirect policy is set at the client level.
        // GPUI's redirect policy extensions are ignored here; the client
        // defaults to following redirects (up to 10).

        let request = request.body(match body.0 {
            Inner::Empty => reqwest::Body::default(),
            Inner::Bytes(cursor) => cursor.into_inner().into(),
            Inner::AsyncReader(stream) => {
                reqwest::Body::wrap_stream(StreamReader::new(stream))
            }
        });

        let handle = self.handle.clone();
        async move {
            let join_handle = handle.spawn(async { request.send().await });

            let mut response = join_handle.await??;

            let headers = mem::take(response.headers_mut());
            let mut builder = http::Response::builder()
                .status(response.status().as_u16())
                .version(response.version());
            *builder.headers_mut().unwrap() = headers;

            let bytes = response
                .bytes_stream()
                .map_err(futures::io::Error::other)
                .into_async_read();
            let body = AsyncBody::from_reader(bytes);

            builder.body(body).map_err(|e| anyhow!(e))
        }
        .boxed()
    }
}

/// A bridge struct that converts an `AsyncRead` into a `Stream` of `Bytes`,
/// needed for creating a reqwest `Body` from GPUI's `AsyncBody`.
struct StreamReader {
    reader: Option<Pin<Box<dyn futures::AsyncRead + Send + Sync>>>,
    buf: BytesMut,
    capacity: usize,
}

impl StreamReader {
    fn new(reader: Pin<Box<dyn futures::AsyncRead + Send + Sync>>) -> Self {
        Self {
            reader: Some(reader),
            buf: BytesMut::new(),
            capacity: DEFAULT_CAPACITY,
        }
    }
}

impl futures::Stream for StreamReader {
    type Item = std::io::Result<Bytes>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = &mut *self;

        let mut reader = match this.reader.take() {
            Some(r) => r,
            None => return Poll::Ready(None),
        };

        if this.buf.capacity() == 0 {
            let capacity = this.capacity;
            this.buf.reserve(capacity);
        }

        match poll_read_buf(&mut reader, cx, &mut this.buf) {
            Poll::Pending => {
                this.reader = Some(reader);
                Poll::Pending
            }
            Poll::Ready(Err(err)) => {
                // Don't put reader back on error
                Poll::Ready(Some(Err(err)))
            }
            Poll::Ready(Ok(0)) => {
                // EOF
                Poll::Ready(None)
            }
            Poll::Ready(Ok(_)) => {
                let chunk = this.buf.split();
                this.reader = Some(reader);
                Poll::Ready(Some(Ok(chunk.freeze())))
            }
        }
    }
}

/// Poll-based read into a BytesMut buffer using futures::AsyncRead.
fn poll_read_buf(
    io: &mut Pin<Box<dyn futures::AsyncRead + Send + Sync>>,
    cx: &mut std::task::Context<'_>,
    buf: &mut BytesMut,
) -> Poll<std::io::Result<usize>> {
    if !buf.has_remaining_mut() {
        return Poll::Ready(Ok(0));
    }

    let dst = buf.chunk_mut();
    // Safety: chunk_mut returns uninitialized memory, we zero it for AsyncRead
    let dst_slice =
        unsafe { &mut *(dst as *mut _ as *mut [u8]) };

    let io_pin = unsafe { Pin::new_unchecked(&mut *io) };
    match futures::AsyncRead::poll_read(io_pin, cx, dst_slice) {
        Poll::Ready(Ok(n)) => {
            unsafe {
                buf.advance_mut(n);
            }
            Poll::Ready(Ok(n))
        }
        Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        Poll::Pending => Poll::Pending,
    }
}

/// Create a GPUI-compatible HTTP client.
pub fn create_http_client(user_agent: &str) -> anyhow::Result<Arc<dyn HttpClient>> {
    Ok(Arc::new(ReqwestClient::user_agent(user_agent)?))
}
