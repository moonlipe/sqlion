//! Vendored change: rustls next to a static OpenSSL, chosen per connection by `Config::legacy_tls`.
//!
//! rustls speaks only TLS 1.2/1.3 with ECDHE suites. Old SQL Server installations (2008/2008 R2/2012 without
//! the TLS updates) offer TLS 1.0/1.1 or RSA key exchange only and close the connection during the handshake
//! (`tls handshake eof`). The legacy connector accepts TLS 1.0+, every suite OpenSSL has (`@SECLEVEL=0`) and
//! servers without secure renegotiation (RFC 5746). The certificate is not checked, like `TrustAll`.
use super::rustls_tls_stream;
use crate::client::config::Config;
use futures_util::io::{AsyncRead, AsyncWrite};
use openssl::ssl::{SslConnector, SslMethod, SslOptions, SslVerifyMode, SslVersion};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};
use tracing::{event, Level};

/// Cipher list of the legacy mode.
pub(crate) const LEGACY_CIPHERS: &str = "ALL:@SECLEVEL=0";

pub(crate) enum TlsStream<S: AsyncRead + AsyncWrite + Unpin + Send> {
    Rustls(rustls_tls_stream::TlsStream<S>),
    OpenSsl(Compat<tokio_openssl::SslStream<Compat<S>>>),
}

fn tls_err(e: impl std::fmt::Display) -> crate::Error {
    crate::Error::Tls(e.to_string())
}

/// `SSL_OP_LEGACY_SERVER_CONNECT` (0x4, not exposed by the `openssl` crate): talk to servers without RFC 5746
/// secure renegotiation, which OpenSSL 3 refuses by default.
fn legacy_options() -> SslOptions {
    SslOptions::from_bits_retain(0x4) | SslOptions::ALLOW_UNSAFE_LEGACY_RENEGOTIATION
}

/// Connector of the legacy mode.
pub(crate) fn legacy_connector() -> crate::Result<SslConnector> {
    let mut b = SslConnector::builder(SslMethod::tls_client()).map_err(tls_err)?;
    b.set_min_proto_version(Some(SslVersion::TLS1)).map_err(tls_err)?;
    b.set_cipher_list(LEGACY_CIPHERS).map_err(tls_err)?;
    b.set_options(legacy_options());
    b.set_verify(SslVerifyMode::NONE);
    Ok(b.build())
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> TlsStream<S> {
    pub(super) async fn new(config: &Config, stream: S) -> crate::Result<Self> {
        if !config.legacy_tls {
            return Ok(TlsStream::Rustls(rustls_tls_stream::TlsStream::new(config, stream).await?));
        }
        event!(Level::INFO, "Performing a legacy TLS handshake (OpenSSL)");
        let ssl = legacy_connector()?
            .configure()
            .map_err(tls_err)?
            .verify_hostname(false)
            .use_server_name_indication(false)
            .into_ssl(config.get_host())
            .map_err(tls_err)?;
        let mut s = tokio_openssl::SslStream::new(ssl, stream.compat()).map_err(tls_err)?;
        Pin::new(&mut s).connect().await.map_err(tls_err)?;
        Ok(TlsStream::OpenSsl(s.compat()))
    }

    pub(crate) fn get_mut(&mut self) -> &mut S {
        match self {
            TlsStream::Rustls(s) => s.get_mut(),
            TlsStream::OpenSsl(s) => s.get_mut().get_mut().get_mut(),
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> AsyncRead for TlsStream<S> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            TlsStream::Rustls(s) => Pin::new(s).poll_read(cx, buf),
            TlsStream::OpenSsl(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> AsyncWrite for TlsStream<S> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            TlsStream::Rustls(s) => Pin::new(s).poll_write(cx, buf),
            TlsStream::OpenSsl(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            TlsStream::Rustls(s) => Pin::new(s).poll_flush(cx),
            TlsStream::OpenSsl(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            TlsStream::Rustls(s) => Pin::new(s).poll_close(cx),
            TlsStream::OpenSsl(s) => Pin::new(s).poll_close(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::{
        asn1::Asn1Time,
        hash::MessageDigest,
        pkey::PKey,
        rsa::Rsa,
        ssl::{SslAcceptor, SslStream},
        x509::{X509NameBuilder, X509},
    };
    use std::net::{TcpListener, TcpStream};

    /// A server that speaks only `version`, with an RSA certificate signed with SHA-1 (like old Windows servers).
    fn old_server(version: SslVersion, ciphers: &str) -> (u16, std::thread::JoinHandle<Result<String, String>>) {
        let key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
        let mut name = X509NameBuilder::new().unwrap();
        name.append_entry_by_text("CN", "old-sql").unwrap();
        let name = name.build();
        let mut cert = X509::builder().unwrap();
        cert.set_version(2).unwrap();
        cert.set_subject_name(&name).unwrap();
        cert.set_issuer_name(&name).unwrap();
        cert.set_pubkey(&key).unwrap();
        cert.set_not_before(&Asn1Time::days_from_now(0).unwrap()).unwrap();
        cert.set_not_after(&Asn1Time::days_from_now(1).unwrap()).unwrap();
        cert.sign(&key, MessageDigest::sha1()).unwrap();
        let cert = cert.build();
        let mut acc = SslAcceptor::mozilla_intermediate(SslMethod::tls_server()).unwrap();
        acc.set_min_proto_version(Some(version)).unwrap();
        acc.set_max_proto_version(Some(version)).unwrap();
        acc.set_cipher_list(ciphers).unwrap();
        acc.set_private_key(&key).unwrap();
        acc.set_certificate(&cert).unwrap();
        let acc = acc.build();
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || {
            let (s, _) = l.accept().map_err(|e| e.to_string())?;
            let s: SslStream<TcpStream> = acc.accept(s).map_err(|e| e.to_string())?;
            Ok(s.ssl().version_str().to_string())
        });
        (port, h)
    }

    fn connect(port: u16) -> Result<String, String> {
        let tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let cfg = legacy_connector().unwrap().configure().unwrap().verify_hostname(false).use_server_name_indication(false);
        cfg.connect("old-sql", tcp).map(|s| s.ssl().version_str().to_string()).map_err(|e| e.to_string())
    }

    #[test]
    fn legacy_connector_talks_to_tls1_rsa_only_servers() {
        // TLS 1.0, static RSA key exchange only (no ECDHE): rustls has neither
        let (port, h) = old_server(SslVersion::TLS1, "AES128-SHA:@SECLEVEL=0");
        assert_eq!(connect(port).unwrap(), "TLSv1");
        assert_eq!(h.join().unwrap().unwrap(), "TLSv1");
        let (port, h) = old_server(SslVersion::TLS1_1, "AES256-SHA:@SECLEVEL=0");
        assert_eq!(connect(port).unwrap(), "TLSv1.1");
        h.join().unwrap().unwrap();
        // static OpenSSL 3 (openssl-src), not the system library
        assert!(openssl::version::number() >= 0x3000_0000, "{}", openssl::version::version());
    }
}
