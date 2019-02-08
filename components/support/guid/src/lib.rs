/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#[cfg(feature = "serde_support")]
mod serde_support;

#[cfg(feature = "rusqlite_support")]
mod rusqlite_support;

use base64::{encode_config_slice, URL_SAFE_NO_PAD};
use rand::{thread_rng, RngCore};

use std::{fmt, ops, str};

/// This is a type intended to be used to represent the guids used by sync. It
/// has several benefits over using a `String`:
///
/// 1. It's more explicit about what is being stored, and could prevent bugs
///    where a Guid is passed to a function expecting text.
///
/// 2. Guids are guaranteed to be immutable.
///
/// 3. It ensures that the value it contains meets what the server considers to
///    be an acceptable record id: The server requires the guids be no more than
///    64 ASCII characters in length, all of which must be between `b' '` and
///    `b'~'`, inclusive.
///
///     - It can also ensure that it's
///
/// 4. It's optimized for the guids commonly used by sync. In particular, short guids
///    (including the guids which would meet `PlacesUtils.isValidGuid`) do not incur
///    any heap allocation, and are stored inline.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Guid(Repr);

// The internal representation of a GUID. Most Sync GUIDs are 12 bytes,
// and contain only base64url characters; we can store them on the stack
// without a heap allocation. However, arbitrary ascii guids of up to length 64
// are possible, in which case we fall back to a heap-allocated string.
//
// This is separate only because making `Guid` an enum would expose the
// internals.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
enum Repr {
    // see FastGuid for invariants
    Fast(FastGuid),

    // invariants:
    // - _0.len() <= MAX_GUID_LEN
    // - _0.bytes().all(|&b| Guid::is_valid_byte(b))
    Slow(String),
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct FastGuid {
    // invariants:
    // - len <= MAX_FAST_GUID_LEN.
    // - data[0..len].iter().all(|&b| Guid::is_valid_byte(b))
    // (bugs causing violation of these invariants could cause panics,
    // not memory unsafety)
    len: u8,
    data: [u8; MAX_FAST_GUID_LEN],
}

// This is the maximum length (experimentally determined) we can make it before
// `Repr::Fast` is larger than `Guid::Slow` on 32 bit systems. The important
// thing is really that it's not too big, and is above 12 bytes.
const MAX_FAST_GUID_LEN: usize = 14;

// Maximum length of a guid accepted by the server.
const MAX_GUID_LEN: usize = 64;

impl FastGuid {
    #[inline]
    fn from_slice(bytes: &[u8]) -> Self {
        // Cecked by the caller, so debug_assert is fine.
        debug_assert_eq!(
            can_use_fast(bytes),
            Ok(true),
            "Bug: Caller failed to check can_use_fast: {:?}",
            bytes
        );
        let mut data = [0u8; MAX_FAST_GUID_LEN];
        data[0..bytes.len()].copy_from_slice(bytes);
        FastGuid {
            len: bytes.len() as u8,
            data,
        }
    }

    #[inline]
    fn as_str(&self) -> &str {
        // Sanity check we weren't mutated and that nobody's creating us in other ways.
        assert_eq!(
            can_use_fast(self.bytes()),
            Ok(true),
            "Bug: FastGuid bytes became invalid: {:?}",
            self.bytes()
        );
        // It would be safe to use str::from_utf8_unchecked here, but we don't
        // to minimize the use of unsafe code and because it seems unthinkable that
        // validating 12 character strings could be a bottleneck.
        str::from_utf8(self.bytes()).unwrap()
    }

    #[inline]
    fn len(&self) -> usize {
        self.len as usize
    }

    #[inline]
    fn bytes(&self) -> &[u8] {
        &self.data[0..self.len()]
    }
}

// Returns:
// - Some(true) to use Repr::Fast
// - Some(false) to use Repr::Slow
// - None if it's never valid
#[inline]
fn can_use_fast<T: ?Sized + AsRef<[u8]>>(bytes: &T) -> Result<bool, GuidError> {
    let bytes = bytes.as_ref();
    if bytes.len() > MAX_GUID_LEN {
        Err(GuidError::TooLong)
    } else if !bytes.iter().all(|&b| Guid::is_valid_byte(b)) {
        Err(GuidError::InvalidBytes)
    } else {
        Ok(bytes.len() <= MAX_FAST_GUID_LEN)
    }
}

impl Guid {
    /// Produces a new places-compatible random Guid.
    pub fn new() -> Guid {
        let mut rng = thread_rng();
        let mut bytes = [0u8; 9];
        // thread_rng *is* a CSPRNG by default (but it's worth noting that that's
        // not really required for generating guids)
        rng.fill_bytes(&mut bytes);
        let mut b64bytes = [0u8; 16];
        let bytes_written = encode_config_slice(&bytes, URL_SAFE_NO_PAD, &mut b64bytes);
        debug_assert_eq!(bytes_written, 12);
        let guid = Guid::try_from_slice(&b64bytes[..bytes_written])
            .expect("Bug: Random guid should be valid");
        debug_assert!(guid.is_places_compatible());
        guid
    }

    /// Try to convert `b` into a `Guid`.
    ///
    /// Returns `Err` if `v.len() >= MAX_GUID_LEN` or the bytes in `v` are not
    /// all valid guid bytes.
    #[inline]
    pub fn try_from_str(s: &str) -> Result<Self, GuidError> {
        Guid::try_from_slice(s.as_ref())
    }

    /// Try to convert `b` into a `Guid`.
    ///
    /// Returns `None` if `v.len() >= MAX_GUID_LEN` or the bytes in `v` are not
    /// all valid guid bytes.
    #[inline]
    pub fn try_from_string(s: String) -> Result<Self, GuidError> {
        Guid::try_from_vec(s.into_bytes())
    }

    /// Try to convert `b` into a `Guid`.
    ///
    /// Returns `None` if `v.len() >= MAX_GUID_LEN` or the bytes in `v` are not
    /// all valid guid bytes.
    #[inline]
    pub fn try_from_slice(b: &[u8]) -> Result<Self, GuidError> {
        can_use_fast(b).map(|can_use| {
            Guid(if can_use {
                Repr::Fast(FastGuid::from_slice(b))
            } else {
                // It would be safe to use from_utf8_unchecked here, as we've
                // already validated the guid.
                Repr::Slow(String::from_utf8(b.into()).expect("Bug: Already validated Guid"))
            })
        })
    }

    /// Try to convert `v` to a `Guid`, consuming it.
    ///
    /// Returns `None` if `v.len() >= MAX_GUID_LEN` or the bytes in `v` are not
    /// all valid guid bytes.
    pub fn try_from_vec(v: Vec<u8>) -> Result<Self, GuidError> {
        can_use_fast(&v).map(|can_use| {
            Guid(if can_use {
                Repr::Fast(FastGuid::from_slice(&v))
            } else {
                Repr::Slow(String::from_utf8(v).expect("Bug: Already validated Guid"))
            })
        })
    }

    /// Try to convert `s` into a guid. Equivalent to unwrapping
    /// [`Guid::try_from_str`]
    ///
    /// # Panics
    ///
    /// Panics if `s.len() >= MAX_GUID_LEN` or the bytes in `s` are not all
    /// valid guid bytes.
    #[inline]
    pub fn from_str(s: &str) -> Self {
        Guid::try_from_str(s).unwrap()
    }

    /// Try to convert `s` into a guid. Equivalent to unwrapping
    /// [`Guid::try_from_slice`]
    ///
    /// # Panics
    ///
    /// Panics if `s.len() >= MAX_GUID_LEN` or the bytes in `s` are not all
    /// valid guid bytes.
    #[inline]
    pub fn from_slice(s: &[u8]) -> Self {
        Guid::try_from_slice(s.as_ref()).unwrap()
    }

    /// Get the data backing this `Guid` as a `&[u8]`.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        match &self.0 {
            Repr::Fast(rep) => rep.bytes(),
            Repr::Slow(rep) => rep.as_ref(),
        }
    }

    /// Get the data backing this `Guid` as a `&str`.
    #[inline]
    pub fn as_str(&self) -> &str {
        match &self.0 {
            Repr::Fast(rep) => rep.as_str(),
            Repr::Slow(rep) => rep.as_ref(),
        }
    }

    /// Convert this `Guid` into a `String`, consuming it in the process.
    #[inline]
    pub fn into_string(self) -> String {
        match self.0 {
            Repr::Fast(rep) => rep.as_str().into(),
            Repr::Slow(rep) => rep,
        }
    }

    /// Returns true for Guids that are valid places guids, and false for all others.
    pub fn is_places_compatible(&self) -> bool {
        self.len() == 12 && self.bytes().all(Guid::is_valid_places_byte)
    }

    /// Returns true if the byte `b` is a character that is allowed to
    /// appear in a GUID.
    #[inline]
    pub fn is_valid_byte(b: u8) -> bool {
        b' ' <= b && b <= b'~'
    }

    /// Returns true if the byte `b` is a valid base64url byte.
    #[inline]
    pub fn is_valid_places_byte(b: u8) -> bool {
        BASE64URL_BYTES[b as usize] == 1
    }

    /// Helper to check that the Guid is places-compatible.
    ///
    /// Useful when chaining on an owned Guid. See `check_places`
    /// for the borrowed version.
    ///
    /// # Example
    /// ```
    /// # use sync_guid::Guid;
    /// // Note: Guid::new always returns a places-compatible Guid,
    /// // so this is unnecessary, it's just a convenient example where
    /// // you have an owned Guid.
    /// let g = Guid::new().ensure_places()?;
    /// ```
    #[inline]
    pub fn ensure_places(self) -> Result<Self, GuidError> {
        self.check_places()?;
        Ok(self)
    }

    /// Return an Err for non-places compatible guids.
    #[inline]
    pub fn check_places(&self) -> Result<&Self, GuidError> {
        if !self.is_places_compatible() {
            Err(GuidError::NotPlacesCompatible)
        } else {
            Ok(self)
        }
    }
}

// This is used to implement the places tests.
const BASE64URL_BYTES: [u8; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0,
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 1,
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Represents a failure to create a Guid, due to it being not allowed
/// on the server, or not being places-compatible when that was requested.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GuidError {
    /// Returned when a guid string is provided which is too
    /// long for the server.
    TooLong,
    /// Returned when the guid string contains invalid bytes
    /// for the server.
    InvalidBytes,
    /// Returned when the guid needs to be places-compatible, but isn't.
    NotPlacesCompatible,
}

impl fmt::Display for GuidError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(<Self as std::error::Error>::description(self))
    }
}

impl std::error::Error for GuidError {
    fn description(&self) -> &'static str {
        match self {
            GuidError::TooLong => "Data is too long to be converted to a guid.",
            GuidError::InvalidBytes => {
                "Data contains bytes that are not allowed to appear in valid Guids."
            }
            GuidError::NotPlacesCompatible => {
                "Guid is not places compatible (12 base64 url-safe characters)"
            }
        }
    }
}

impl<'a> From<&'a str> for Guid {
    #[inline]
    fn from(s: &'a str) -> Guid {
        Guid::from_str(s)
    }
}

impl<'a> From<&'a [u8]> for Guid {
    #[inline]
    fn from(s: &'a [u8]) -> Guid {
        Guid::from_slice(s)
    }
}

impl From<String> for Guid {
    #[inline]
    fn from(s: String) -> Guid {
        Guid::try_from_string(s).unwrap()
    }
}

impl From<Vec<u8>> for Guid {
    #[inline]
    fn from(v: Vec<u8>) -> Guid {
        Guid::try_from_vec(v).unwrap()
    }
}

impl From<Guid> for String {
    #[inline]
    fn from(guid: Guid) -> String {
        guid.into_string()
    }
}

impl From<Guid> for Vec<u8> {
    #[inline]
    fn from(guid: Guid) -> Vec<u8> {
        guid.into_string().into_bytes()
    }
}

impl AsRef<str> for Guid {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<[u8]> for Guid {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl ops::Deref for Guid {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

// The default Debug impl is pretty unhelpful here.
impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Guid({:?})", self.as_str())
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl std::hash::Hash for Guid {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state)
    }
}

macro_rules! impl_guid_eq {
    ($($other: ty),+) => {$(
        impl<'a> PartialEq<$other> for Guid {
            #[inline]
            fn eq(&self, other: &$other) -> bool {
                PartialEq::eq(AsRef::<[u8]>::as_ref(self), AsRef::<[u8]>::as_ref(other))
            }

            #[inline]
            fn ne(&self, other: &$other) -> bool {
                PartialEq::ne(AsRef::<[u8]>::as_ref(self), AsRef::<[u8]>::as_ref(other))
            }
        }

        impl<'a> PartialEq<Guid> for $other {
            #[inline]
            fn eq(&self, other: &Guid) -> bool {
                PartialEq::eq(AsRef::<[u8]>::as_ref(self), AsRef::<[u8]>::as_ref(other))
            }

            #[inline]
            fn ne(&self, other: &Guid) -> bool {
                PartialEq::ne(AsRef::<[u8]>::as_ref(self), AsRef::<[u8]>::as_ref(other))
            }
        }
    )+}
}

// Implement direct comparison with some common types from the stdlib.
impl_guid_eq![str, &'a str, String, [u8], &'a [u8], Vec<u8>];

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_base64url_bytes() {
        let mut expect = [0u8; 256];
        for b in b'0'..=b'9' {
            expect[b as usize] = 1;
        }
        for b in b'a'..=b'z' {
            expect[b as usize] = 1;
        }
        for b in b'A'..=b'Z' {
            expect[b as usize] = 1;
        }
        expect[b'_' as usize] = 1;
        expect[b'-' as usize] = 1;
        assert_eq!(&BASE64URL_BYTES[..], &expect[..]);
    }

    #[test]
    fn test_valid_for_places() {
        assert!(Guid::from("aaaabbbbcccc").is_places_compatible());
        assert!(Guid::from_slice(b"09_az-AZ_09-").is_places_compatible());
        assert!(!Guid::from("aaaabbbbccccd").is_places_compatible()); // too long
        assert!(!Guid::from("aaaabbbbccc").is_places_compatible()); // too short
        assert!(!Guid::from("aaaabbbbccc=").is_places_compatible()); // right length, bad character
    }

    #[test]
    fn test_comparison() {
        assert_eq!(Guid::from("abcdabcdabcd"), "abcdabcdabcd");
        assert_ne!(Guid::from("abcdabcdabcd".to_string()), "ABCDabcdabcd");

        assert_eq!(Guid::from("abcdabcdabcd"), &b"abcdabcdabcd"[..]); // b"abcdabcdabcd" has type &[u8; 12]...
        assert_ne!(Guid::from(&b"abcdabcdabcd"[..]), &b"ABCDabcdabcd"[..]);

        assert_eq!(
            Guid::from("abcdabcdabcd".as_bytes().to_owned()),
            "abcdabcdabcd".to_string()
        );
        assert_ne!(Guid::from("abcdabcdabcd"), "ABCDabcdabcd".to_string());

        assert_eq!(
            Guid::from("abcdabcdabcd1234"),
            Vec::from(b"abcdabcdabcd1234".as_ref())
        );
        assert_ne!(
            Guid::from("abcdabcdabcd4321"),
            Vec::from(b"ABCDabcdabcd4321".as_ref())
        );
    }

    #[test]
    fn test_guid_validation() {
        assert!(Guid::try_from_string("a".repeat(64)).is_ok());
        assert!(Guid::try_from_string("a".repeat(64)).is_ok());
        assert_eq!(
            Guid::try_from_string("a".repeat(65)),
            Err(GuidError::TooLong)
        );
        assert_eq!(Guid::try_from_str(" foo~").unwrap(), " foo~");

        // after '~'
        assert_eq!(
            Guid::try_from_slice(b"12345\x7f"),
            Err(GuidError::InvalidBytes)
        );
        // before ' '
        assert_eq!(
            Guid::try_from_slice(b"a\x1fbc12345"),
            Err(GuidError::InvalidBytes)
        );
        assert_eq!(Guid::try_from_str("aaaaa\n"), Err(GuidError::InvalidBytes));

        // All unicode should be prevented
        assert_eq!(Guid::try_from_str("fööbar"), Err(GuidError::InvalidBytes));
        assert_eq!(Guid::try_from_str("em😺ji"), Err(GuidError::InvalidBytes));

        // Invalid utf8
        assert_eq!(
            Guid::try_from_slice(b"aaaabbbbccc\xa0"),
            Err(GuidError::InvalidBytes)
        );
    }

    #[test]
    fn test_new() {
        let mut seen = HashSet::new();
        for _ in 0..100 {
            let g = Guid::new();
            assert_eq!(g.len(), 12);
            assert!(g.is_places_compatible());
            assert!(!seen.contains(&g));
            seen.insert(g);
        }
    }
}
