# Local TLS fixture

Generated solely for loopback software tests; contains no study data or production keys.
The self-signed certificate is deliberately absent from the production WebPKI roots.
A test-only agent trusts it to exercise an HTTPS-to-HTTP redirect refusal.

```sh
openssl req -x509 -newkey rsa:2048 -nodes -keyout localhost.key -out localhost.crt \
  -days 36500 -subj '/CN=localhost' \
  -addext 'subjectAltName=IP:127.0.0.1,DNS:localhost' \
  -addext 'basicConstraints=critical,CA:FALSE'
openssl x509 -in localhost.crt -outform DER -out localhost.der
openssl pkcs8 -topk8 -nocrypt -in localhost.key -outform DER -out localhost-key.der
```
