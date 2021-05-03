//! Lexicographical counter.

use std::fmt;

/// A counter which prints numbers in lexicographic order and smaller numbers
/// are also shorter.
///
/// The output sequence is like:
///  * 000, 001, …, 099,
///  * 10000, 10001, …, 19999,
///  * 2000000, …, 2999999,
///  * …
///  * 900000000000000000000, …, 999999999999999999999.
///
/// It can count up to 10^(20) distinct numbers.
#[derive(Debug, Copy, Clone)]
pub struct LexCtr {
    prefix: usize,
    count: u64,
    limit: u64,
}

impl Default for LexCtr {
    fn default() -> Self {
        Self {
            prefix: 0,
            count: 0,
            limit: 100,
        }
    }
}

impl LexCtr {
    /// Increases the counter by 1.
    ///
    /// # Panics
    ///
    /// Panics if the count exceeds 10^(20).
    pub fn inc(&mut self) {
        self.count += 1;
        if self.count >= self.limit {
            self.limit *= 100;
            self.prefix += 1;
            self.count = 0;
        }
    }
}

impl fmt::Display for LexCtr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{0}{2:01$}", self.prefix, self.prefix * 2 + 2, self.count)
    }
}

#[test]
fn test_lexctr() {
    let mut lexctr = LexCtr::default();
    assert_eq!(lexctr.to_string(), "000");
    lexctr.inc();
    assert_eq!(lexctr.to_string(), "001");
    for _ in 1..99 {
        lexctr.inc();
    }
    assert_eq!(lexctr.to_string(), "099");
    lexctr.inc();
    assert_eq!(lexctr.to_string(), "10000");
    for _ in 10000..19999 {
        lexctr.inc();
    }
    assert_eq!(lexctr.to_string(), "19999");
    lexctr.inc();
    assert_eq!(lexctr.to_string(), "2000000");
}
