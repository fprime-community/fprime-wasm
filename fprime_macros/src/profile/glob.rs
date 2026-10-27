//! The name patterns a [`Select`](super::config::Select) chooses rules by.

use serde::Deserialize;

/// A name where `*` matches any run of characters, `.` and `::` included — so `Ref.*`
/// covers every point under `Ref`.
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct Glob(String);

impl Glob {
    pub fn matches(&self, name: &str) -> bool {
        let (pattern, name) = (self.0.as_bytes(), name.as_bytes());
        let (mut p, mut n) = (0, 0);
        // The last `*` seen, and where in `name` it began matching — to backtrack to.
        let mut star = None;

        while n < name.len() {
            if p < pattern.len() && pattern[p] == b'*' {
                star = Some((p, n));
                p += 1;
            } else if p < pattern.len() && pattern[p] == name[n] {
                p += 1;
                n += 1;
            } else if let Some((star_p, star_n)) = star {
                p = star_p + 1;
                n = star_n + 1;
                star = Some((star_p, n));
            } else {
                return false;
            }
        }

        pattern[p..].iter().all(|&byte| byte == b'*')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glob(pattern: &str) -> Glob {
        Glob(pattern.into())
    }

    #[test]
    fn matches() {
        assert!(glob("Ref.*").matches("Ref.power.PWR_OFF"));
        assert!(glob("*.PWR_*").matches("Ref.power.PWR_OFF"));
        assert!(glob("Ref.power.PWR_OFF").matches("Ref.power.PWR_OFF"));
        assert!(glob("*").matches(""));
        assert!(!glob("Ref.*").matches("CdhCore.cmdDisp.CMD_NO_OP"));
        assert!(!glob("Ref.power").matches("Ref.power.PWR_OFF"));
        assert!(!glob("*_ON").matches("Ref.power.PWR_OFF"));
    }
}
