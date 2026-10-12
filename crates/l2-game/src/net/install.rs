//! C257: an id each install makes once and keeps in `prefs.ini`; a network save
//! records which install played which realm. Data only: it is not on the wire yet.
//! Not a secret (`docs/netcode.md` §8: v1 has no authentication).

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct InstallId(pub [u8; 16]);

impl InstallId {
    pub const NIL: InstallId = InstallId([0; 16]);

    pub fn is_nil(&self) -> bool {
        *self == InstallId::NIL
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn from_hex(s: &str) -> Option<InstallId> {
        let s = s.trim();
        if s.len() != 32 || !s.is_ascii() {
            return None;
        }
        let mut out = [0u8; 16];
        for (i, b) in out.iter_mut().enumerate() {
            *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(InstallId(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ablation: the length check in `from_hex` (a short or long value parses).
    #[test]
    fn an_id_round_trips_through_its_hex_and_nothing_else_parses() {
        let id = InstallId([0x0F, 0xA5, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 0xFF]);
        assert_eq!(InstallId::from_hex(&id.to_hex()), Some(id));
        assert_eq!(InstallId::from_hex("0fa5"), None);
        assert_eq!(InstallId::from_hex(&format!("{}00", id.to_hex())), None);
        assert_eq!(InstallId::from_hex(&"zz".repeat(16)), None);
    }
}
