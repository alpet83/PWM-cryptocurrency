pub fn delegation_ping() -> &'static str {
    "ok-v3"
}

pub fn delegation_version() -> u32 {
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delegation_ping_returns_ok_v3() {
        assert_eq!(delegation_ping(), "ok-v3");
    }

    #[test]
    fn delegation_version_returns_3() {
        assert_eq!(delegation_version(), 3);
    }
}
