//! The site name (SNI) a TLS ClientHello carries in clear.

use crate::constants::{
    CLIENT_HELLO, EXT_SERVER_NAME, HANDSHAKE_LENGTH, HELLO_FIXED, HEX_RADIX, HEX_SEP, HOST_MAX,
    NAME_LIST_LENGTH, NAME_TYPE_HOST, RECORD_HEADER, TLS_HANDSHAKE,
};

/// Bytes printed by bpftrace's %rh: two hex digits each, spaces between.
pub fn from_hex(text: &str) -> Option<Vec<u8>> {
    let bytes = text
        .trim()
        .split(HEX_SEP)
        .map(|byte| u8::from_str_radix(byte, HEX_RADIX).ok());
    bytes.collect()
}

/// Reads a ClientHello front to back; every read fails cleanly past the end.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let (head, rest) = self.0.split_at_checked(count)?;
        self.0 = rest;
        Some(head)
    }

    fn byte(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn short(&mut self) -> Option<usize> {
        let bytes = self.take(2)?;
        Some(usize::from(u16::from_be_bytes([bytes[0], bytes[1]])))
    }

    /// A part that starts with its length in one or two bytes.
    fn part(&mut self, length_bytes: usize) -> Option<Reader<'a>> {
        let length = if length_bytes == 1 {
            usize::from(self.byte()?)
        } else {
            self.short()?
        };
        Some(Reader(self.take(length)?))
    }
}

/// The host name in a ClientHello's server_name extension; None for anything else.
pub fn server_name(hello: &[u8]) -> Option<String> {
    let mut reader = Reader(hello);
    let header = reader.take(RECORD_HEADER)?;
    if header[0] != TLS_HANDSHAKE || reader.byte()? != CLIENT_HELLO {
        return None;
    }
    reader.take(HANDSHAKE_LENGTH + HELLO_FIXED)?;
    reader.part(1)?; // session id
    reader.part(2)?; // cipher suites
    reader.part(1)?; // compression methods
    let mut extensions = reader.part(2)?;
    while let Some(kind) = extensions.short() {
        let mut data = extensions.part(2)?;
        if kind == usize::from(EXT_SERVER_NAME) {
            data.take(NAME_LIST_LENGTH)?;
            if data.byte()? != NAME_TYPE_HOST {
                return None;
            }
            let name = std::str::from_utf8(data.part(2)?.0).ok()?;
            return host_like(name).then(|| name.to_ascii_lowercase());
        }
    }
    None
}

/// Letters, digits, dots, dashes and underscores only: nothing an agent could use to fake a log line.
fn host_like(name: &str) -> bool {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_');
    !name.is_empty() && name.len() <= HOST_MAX && name.chars().all(allowed)
}

#[cfg(test)]
#[path = "sni_tests.rs"]
mod tests;
