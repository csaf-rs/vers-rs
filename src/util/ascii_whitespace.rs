/// [`u8::is_ascii_whitespace`] doesn't cover 0x0B so we define our own array of ASCII whitespace characters
const ASCII_WHITESPACE_CHARACTERS: [u8; 6] = [b' ', b'\t', b'\n', b'\r', 0x0B, 0x0C];

/// A 256-entry byte map indexed by byte value, built once at compile time, so checking whether a
/// byte is ASCII whitespace is an O(1) lookup instead of scanning [`ASCII_WHITESPACE_CHARACTERS`].
const ASCII_WHITESPACE_BYTEMAP: [bool; 256] = {
    let mut map = [false; 256];
    let mut i = 0;
    while i < ASCII_WHITESPACE_CHARACTERS.len() {
        map[ASCII_WHITESPACE_CHARACTERS[i] as usize] = true;
        i += 1;
    }
    map
};

/// Returns whether `b` is an ASCII whitespace or control character disallowed in a VERS string.
///
/// The check recognizes space, horizontal tab, line feed, carriage return, vertical tab, and
/// form feed. Non-ASCII bytes and all other ASCII bytes return `false`.
#[inline]
pub(crate) const fn is_ascii_vers_whitespace(b: u8) -> bool {
    ASCII_WHITESPACE_BYTEMAP[b as usize]
}

#[cfg(test)]
mod tests {
    use super::is_ascii_vers_whitespace;
    use rstest::rstest;

    #[rstest]
    #[case(b' ', true)]
    #[case(b'\t', true)]
    #[case(b'\n', true)]
    #[case(b'\r', true)]
    #[case(0x0B, true)]
    #[case(0x0C, true)]
    #[case(b'a', false)]
    fn test_is_ascii_vers_whitespace(#[case] input: u8, #[case] expected: bool) {
        assert_eq!(is_ascii_vers_whitespace(input), expected);
    }
}
