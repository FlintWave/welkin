// SPDX-License-Identifier: LGPL-3.0-or-later

//! OpenSim Profile Anchor (OSPA) resolver.
//!
//! An OSPA is a string used to identify user profiles across grids.
//! Format: `ospa:n=First Last`
//!
//! Ported from `OpenSim.Framework.Serialization.OspResolver`.

use uuid::Uuid;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Prefix that identifies an OSPA string.
pub const OSPA_PREFIX: &str = "ospa:";

/// Key for the name tuple in an OSPA.
const OSPA_NAME_KEY: &str = "n";

/// Separator between first and last name in the OSPA name value.
const OSPA_NAME_VALUE_SEPARATOR: char = ' ';

/// Separator between tuples in an OSPA string.
const OSPA_TUPLE_SEPARATOR: char = '|';

/// Separator between key and value in an OSPA tuple.
const OSPA_PAIR_SEPARATOR: char = '=';

// ---------------------------------------------------------------------------
// User service trait
// ---------------------------------------------------------------------------

/// Abstraction for looking up users by name or UUID.
///
/// Implementations connect to the grid's user-account service.
pub trait UserService {
    /// Look up a user by first and last name, returning their UUID if found.
    fn get_user_by_name(&self, first: &str, last: &str) -> Option<Uuid>;

    /// Look up a user by UUID, returning `(first_name, last_name)` if found.
    fn get_user_by_id(&self, id: Uuid) -> Option<(String, String)>;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Create an OSPA string from a user UUID, using the user service to resolve
/// the name. Returns `None` if the user cannot be found.
pub fn make_ospa_from_uuid(user_id: Uuid, user_service: &dyn UserService) -> Option<String> {
    let (first, last) = user_service.get_user_by_id(user_id)?;
    Some(make_ospa(&first, &last))
}

/// Create an OSPA string from a first and last name.
///
/// Format: `ospa:n=First Last`
pub fn make_ospa(first: &str, last: &str) -> String {
    format!(
        "{OSPA_PREFIX}{OSPA_NAME_KEY}{}{first}{}{last}",
        OSPA_PAIR_SEPARATOR, OSPA_NAME_VALUE_SEPARATOR,
    )
}

/// Hash a profile name into a deterministic UUID.
///
/// Matches the C# `OspResolver.HashName` behaviour:
/// MD5 of the UTF-16LE–encoded name bytes, interpreted as a big-endian UUID
/// (OpenMetaverse network-order convention).
pub fn hash_name(name: &str) -> Uuid {
    let utf16le_bytes: Vec<u8> = name.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    let digest = md5::compute(&utf16le_bytes);
    Uuid::from_bytes(digest.0)
}

/// Resolve an OSPA string to a user UUID.
///
/// If the OSPA names a user that exists on this grid, their real UUID is
/// returned. Otherwise `None` is returned.
///
/// Non-OSPA strings (those not starting with `ospa:`) also return `None`.
pub fn resolve_ospa(ospa: &str, user_service: &dyn UserService) -> Option<Uuid> {
    let meat = ospa.strip_prefix(OSPA_PREFIX)?;

    for tuple in meat.split(OSPA_TUPLE_SEPARATOR) {
        let sep_idx = tuple.find(OSPA_PAIR_SEPARATOR)?;
        let key = tuple[..sep_idx].trim();
        let value = tuple[sep_idx + 1..].trim();

        if key == OSPA_NAME_KEY {
            return resolve_ospa_name(value, user_service);
        }
    }

    None
}

/// Resolve an OSPA name value (`"First Last"`) to a UUID by querying the
/// user service.
///
/// **Security**: this never *creates* user mappings — it only looks up
/// existing accounts. Callers must not allow untrusted OSPA names to
/// overwrite existing grid users.
fn resolve_ospa_name(name: &str, user_service: &dyn UserService) -> Option<Uuid> {
    let sep_idx = name.find(OSPA_NAME_VALUE_SEPARATOR)?;
    let first = name[..sep_idx].trim_end();
    let last = name[sep_idx + 1..].trim_start();

    if first.is_empty() || last.is_empty() {
        return None;
    }

    user_service.get_user_by_name(first, last)
}

// ---------------------------------------------------------------------------
// OspResolver struct — caches name→UUID mappings
// ---------------------------------------------------------------------------

/// Caching resolver that remembers name→UUID lookups.
///
/// Used during OAR/IAR load to avoid redundant user-service queries.
#[derive(Debug, Clone)]
pub struct OspResolverCache {
    cache: std::collections::HashMap<String, Option<Uuid>>,
}

impl OspResolverCache {
    /// Create a new, empty resolver cache.
    pub fn new() -> Self {
        Self {
            cache: std::collections::HashMap::new(),
        }
    }

    /// Resolve an OSPA, using the cache to avoid redundant lookups.
    pub fn resolve(&mut self, ospa: &str, user_service: &dyn UserService) -> Option<Uuid> {
        let key = ospa.to_string();
        if let Some(cached) = self.cache.get(&key) {
            return *cached;
        }
        let result = resolve_ospa(ospa, user_service);
        self.cache.insert(key, result);
        result
    }

    /// Check if a user already exists before creating a mapping.
    ///
    /// **Security**: UUID injection prevention — OSPA names from untrusted
    /// archives must not overwrite existing grid users. This method returns
    /// `true` if the user already exists and the mapping should be skipped.
    pub fn user_exists(&self, first: &str, last: &str, user_service: &dyn UserService) -> bool {
        user_service.get_user_by_name(first, last).is_some()
    }
}

impl Default for OspResolverCache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock user service for testing.
    struct MockUserService {
        users: Vec<(String, String, Uuid)>,
    }

    impl MockUserService {
        fn new() -> Self {
            Self { users: Vec::new() }
        }

        fn add_user(&mut self, first: &str, last: &str, id: Uuid) {
            self.users.push((first.to_string(), last.to_string(), id));
        }
    }

    impl UserService for MockUserService {
        fn get_user_by_name(&self, first: &str, last: &str) -> Option<Uuid> {
            self.users
                .iter()
                .find(|(f, l, _)| f == first && l == last)
                .map(|(_, _, id)| *id)
        }

        fn get_user_by_id(&self, id: Uuid) -> Option<(String, String)> {
            self.users
                .iter()
                .find(|(_, _, uid)| *uid == id)
                .map(|(f, l, _)| (f.clone(), l.clone()))
        }
    }

    #[test]
    fn make_ospa_format() {
        let ospa = make_ospa("Test", "User");
        assert_eq!(ospa, "ospa:n=Test User");
    }

    #[test]
    fn make_ospa_from_uuid_found() {
        let uuid = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let mut svc = MockUserService::new();
        svc.add_user("Jane", "Doe", uuid);
        let result = make_ospa_from_uuid(uuid, &svc);
        assert_eq!(result, Some("ospa:n=Jane Doe".to_string()));
    }

    #[test]
    fn make_ospa_from_uuid_not_found() {
        let uuid = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let svc = MockUserService::new();
        let result = make_ospa_from_uuid(uuid, &svc);
        assert!(result.is_none());
    }

    #[test]
    fn hash_name_deterministic() {
        let h1 = hash_name("Test User");
        let h2 = hash_name("Test User");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_name_different_names_differ() {
        let h1 = hash_name("Test User");
        let h2 = hash_name("Other Person");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_name_matches_csharp_utf16le() {
        // Verify the hash uses UTF-16LE encoding, not UTF-8.
        // "A" in UTF-8 is [0x41], in UTF-16LE is [0x41, 0x00].
        // If we accidentally used UTF-8, the hash would differ.
        let h_a = hash_name("A");

        // Manually compute: MD5 of UTF-16LE bytes of "A" = [0x41, 0x00]
        let utf16le: Vec<u8> = vec![0x41, 0x00];
        let expected = md5::compute(&utf16le);
        let expected_uuid = Uuid::from_bytes(expected.0);
        assert_eq!(h_a, expected_uuid);

        // Make sure it's NOT the UTF-8 hash
        let utf8_hash = md5::compute(b"A");
        let utf8_uuid = Uuid::from_bytes(utf8_hash.0);
        assert_ne!(h_a, utf8_uuid, "hash should use UTF-16LE, not UTF-8");
    }

    #[test]
    fn resolve_ospa_known_user() {
        let uuid = Uuid::parse_str("abcdef01-2345-6789-abcd-ef0123456789").unwrap();
        let mut svc = MockUserService::new();
        svc.add_user("Test", "User", uuid);

        let result = resolve_ospa("ospa:n=Test User", &svc);
        assert_eq!(result, Some(uuid));
    }

    #[test]
    fn resolve_ospa_unknown_user() {
        let svc = MockUserService::new();
        let result = resolve_ospa("ospa:n=Nobody Here", &svc);
        assert!(result.is_none());
    }

    #[test]
    fn resolve_ospa_not_ospa_prefix() {
        let svc = MockUserService::new();
        let result = resolve_ospa("not-an-ospa-string", &svc);
        assert!(result.is_none());
    }

    #[test]
    fn resolve_ospa_malformed_name() {
        let svc = MockUserService::new();
        // No space separator between first and last
        let result = resolve_ospa("ospa:n=SingleName", &svc);
        assert!(result.is_none());
    }

    #[test]
    fn uuid_injection_prevention() {
        // Verify that existing users are not overwritten by untrusted OSPA names.
        let existing_uuid = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let mut svc = MockUserService::new();
        svc.add_user("Admin", "User", existing_uuid);

        let cache = OspResolverCache::new();

        // The user already exists — should not be overwritten
        assert!(cache.user_exists("Admin", "User", &svc));

        // A non-existing user should return false
        assert!(!cache.user_exists("New", "Person", &svc));
    }

    #[test]
    fn cache_avoids_redundant_lookups() {
        let uuid = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        let mut svc = MockUserService::new();
        svc.add_user("Cached", "User", uuid);

        let mut cache = OspResolverCache::new();

        // First call — cache miss
        let r1 = cache.resolve("ospa:n=Cached User", &svc);
        assert_eq!(r1, Some(uuid));

        // Second call — cache hit (same result)
        let r2 = cache.resolve("ospa:n=Cached User", &svc);
        assert_eq!(r2, Some(uuid));
    }
}
