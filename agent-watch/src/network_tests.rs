//! Network records: every contact kept, and TLS site names.

use serde_json::json;

use super::tests::{events, printf};

const CURL: &str = include_str!("fixtures/curl-hello.hex");

fn tls(hello: &str) -> String {
    printf("tls", 9, 7, "curl", &["443", "93.184.215.14", hello.trim()])
}

#[test]
fn a_hello_gives_the_site_its_address_and_port() {
    let records = events().handle(&tls(CURL));
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(
        (
            &record["kind"],
            &record["host"],
            &record["addr"],
            &record["port"]
        ),
        (
            &json!("tls"),
            &json!("example.org"),
            &json!("93.184.215.14"),
            &json!(443)
        )
    );
}

#[test]
fn a_hello_without_a_site_name_gives_nothing() {
    assert!(events().handle(&tls("16 03 01 00")).is_empty());
}

#[test]
fn repeated_contacts_are_all_kept() {
    let mut events = events();
    let lookup = printf("dns", 9, 7, "curl", &["example.org"]);
    let connect = printf("connect", 9, 7, "curl", &["443", "93.184.215.14"]);
    for line in [&lookup, &connect, &tls(CURL)] {
        assert_eq!(events.handle(line).len(), 1);
        assert_eq!(events.handle(line).len(), 1);
    }
}
