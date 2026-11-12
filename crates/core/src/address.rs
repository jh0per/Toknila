//! What a valid address file (`daemon.json`) is: the rule clients apply before they trust it.

/// What an address file says, as far as the rule needs.
#[derive(Debug, Clone, Copy)]
pub struct Address<'a> {
    pub port: u16,
    pub token: &'a str,
}

/// Why an address file cannot be used yet.
pub const INCOMPLETE: &str = "the address file is incomplete";

/// Whether the address can be dialled: a port, and a token that fits an HTTP header value.
pub fn is_complete(a: Address<'_>) -> bool {
    a.port != 0 && !a.token.is_empty() && a.token.bytes().all(|b| b.is_ascii_graphic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_port_and_a_printable_token_make_a_complete_address() {
        assert!(is_complete(Address {
            port: 4100,
            token: "ab12"
        }));
    }

    #[test]
    fn a_missing_port_or_a_cut_token_is_incomplete() {
        for (port, token) in [(0, "ab12"), (4100, ""), (4100, "ab\n12"), (4100, "a b")] {
            assert!(!is_complete(Address { port, token }), "{port} {token:?}");
        }
    }
}
