use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use derive_more::with_trait::Debug;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};

pub(super) static RELOAD_NOTIFY: std::sync::OnceLock<Arc<tokio::sync::Notify>> =
    std::sync::OnceLock::new();

/// A wrapper over [`TcpListener`] that can reset existing TCP connections when
/// a globally defined signal is set.
///
/// This is useful for hot-patching, so that the clients that are already
/// connected won't use the connection to the old code and will use the
/// hotpatched versions of the handlers instead.
#[derive(Debug)]
pub(crate) struct ResetListener {
    inner: TcpListener,
}

impl ResetListener {
    pub(crate) fn new(inner: TcpListener) -> Self {
        Self { inner }
    }
}

impl axum::serve::Listener for ResetListener {
    type Io = ResetStream;
    type Addr = std::net::SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match self.inner.accept().await {
                Ok((stream, addr)) => {
                    let notify = RELOAD_NOTIFY.get_or_init(|| Arc::new(tokio::sync::Notify::new()));
                    let notify = notify.clone();
                    return (
                        ResetStream {
                            inner: stream,
                            reset_fut: Box::pin(async move { notify.notified().await }),
                        },
                        addr,
                    );
                }
                Err(_err) => {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.inner.local_addr()
    }
}

#[derive(Debug)]
pub(crate) struct ResetStream {
    inner: TcpStream,
    #[debug("..")]
    reset_fut: Pin<Box<dyn Future<Output = ()> + Send>>,
}

impl ResetStream {
    fn forward_to_inner<T, F>(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        f: F,
    ) -> Poll<std::io::Result<T>>
    where
        F: FnOnce(Pin<&mut TcpStream>, &mut Context<'_>) -> Poll<std::io::Result<T>>,
    {
        if self.reset_fut.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "connection reset by live reload",
            )));
        }
        f(Pin::new(&mut self.inner), cx)
    }
}

impl AsyncRead for ResetStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.forward_to_inner(cx, |inner, cx| inner.poll_read(cx, buf))
    }
}

impl AsyncWrite for ResetStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.forward_to_inner(cx, |inner, cx| inner.poll_write(cx, buf))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.forward_to_inner(cx, AsyncWrite::poll_flush)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.forward_to_inner(cx, AsyncWrite::poll_shutdown)
    }
}
