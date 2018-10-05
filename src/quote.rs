#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Quote {
    /// Quote a string using `'…'`.
    Single = b'\'',

    /// Quote an identifier using `"…"`. This is the ISO SQL standard symbol,
    /// and is supported in most SQL dialects e.g. PostgreSQL.
    Double = b'"',

    /// Quote an identifier using `` `…` ``. This is mainly used in MySQL.
    Backquote = b'`',

    /// Quote an identifier using `[…]`. This is mainly used in Microsoft Transact-SQL.
    Brackets = b'[',
}

impl Quote {
    pub fn escape(self, identifier: &str) -> String {
        let res = self.escape_bytes(identifier.as_bytes());
        unsafe { String::from_utf8_unchecked(res) }
    }

    pub fn escape_bytes(self, identifier: &[u8]) -> Vec<u8> {
        let mut res = Vec::with_capacity(identifier.len() + 2);
        res.push(self as u8);
        for b in identifier {
            res.push(*b);
            if self != Quote::Brackets && *b == self as u8 {
                res.push(self as u8);
            }
        }
        res.push(if self == Quote::Brackets { b']' } else { self as u8 });
        res
    }

    pub fn unescape(self, quoted: &str) -> String {
        let middle = &quoted[1..quoted.len() - 1];
        let (one, two) = match self {
            Quote::Single => ('\'', "''"),
            Quote::Double => ('"', "\"\""),
            Quote::Backquote => ('`', "``"),
            Quote::Brackets => return middle.to_owned(),
        };
        middle.replace(one, two)
    }
}
