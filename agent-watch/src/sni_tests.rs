//! Site names from real ClientHellos (curl and Chrome, captured against a local listener) and broken ones.

use super::{from_hex, server_name};

const CURL: &str = include_str!("fixtures/curl-hello.hex");
const CHROME: &str = include_str!("fixtures/chrome-hello.hex");

#[test]
fn curl_and_chrome_name_the_site() {
    for hello in [CURL, CHROME] {
        let bytes = from_hex(hello).unwrap();
        assert_eq!(server_name(&bytes).as_deref(), Some("example.org"));
    }
}

#[test]
fn cut_short_or_not_a_hello_gives_none() {
    let bytes = from_hex(CHROME).unwrap();
    assert_eq!(server_name(&bytes[..100]), None);
    assert_eq!(server_name(&bytes[1..]), None);
    assert_eq!(server_name(b"GET / HTTP/1.1"), None);
    assert_eq!(from_hex("16 zz"), None);
}
