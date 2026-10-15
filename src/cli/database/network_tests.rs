use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
};
const CERT: &[u8] = include_bytes!("testdata/localhost.der");
const KEY: &[u8] = include_bytes!("testdata/localhost-key.der");
fn server(response: String) -> (String, thread::JoinHandle<()>) {
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CERT.to_vec().into()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(KEY.to_vec()).into(),
            )
            .unwrap();
        let (stream, _) = socket.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut tls = rustls::StreamOwned::new(
            rustls::ServerConnection::new(Arc::new(config)).unwrap(),
            stream,
        );
        let mut request = vec![];
        let mut byte = [0];
        while request.len() < 8192 {
            if tls.read_exact(&mut byte).is_err() {
                return;
            }
            request.push(byte[0]);
            if request.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let _ = tls.write_all(response.as_bytes());
        let _ = tls.flush();
    });
    (format!("https://{address}/data"), handle)
}
#[test]
fn tls_verification_and_https_only_redirect_policy_are_enforced() {
    let (url, worker) = server("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".into());
    let result = agent().get(url).config().proxy(None).build().call();
    assert!(
        matches!(&result,Err(ureq::Error::Io(e)) if e.kind() == io::ErrorKind::InvalidData),
        "untrusted certificate: {result:?}"
    );
    assert!(result.unwrap_err().to_string().contains("UnknownIssuer"));
    worker.join().unwrap();
    let sink = TcpListener::bind("127.0.0.1:0").unwrap();
    sink.set_nonblocking(true).unwrap();
    let (url, worker) = server(format!(
        "HTTP/1.1 302 Found\r\nLocation: http://{}/plain\r\nContent-Length: 0\r\n\r\n",
        sink.local_addr().unwrap()
    ));
    // Only this local test trusts the generated server certificate. Production uses WebPKI.
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::new_with_certs(&[
            ureq::tls::Certificate::from_der(CERT),
        ]))
        .build();
    let result = agent()
        .get(url)
        .config()
        .proxy(None)
        .tls_config(tls)
        .build()
        .call();
    assert!(
        matches!(result, Err(ureq::Error::RequireHttpsOnly(_))),
        "downgrade: {result:?}"
    );
    worker.join().unwrap();
    assert_eq!(sink.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
}
