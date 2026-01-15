use rustc_hash::FxHashMap;

/// A string interner that deduplicates strings and assigns them unique IDs
/// This reduces memory usage when the same strings are used repeatedly (like identifiers)
#[derive(Debug, Default)]
pub struct StringInterner {
    /// Map from string to its ID
    string_to_id: FxHashMap<String, StringId>,
    /// Map from ID to string
    id_to_string: Vec<String>,
}

/// A unique identifier for an interned string
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StringId(u32);

impl StringInterner {
    /// Create a new string interner
    pub fn new() -> Self {
        Self {
            string_to_id: FxHashMap::default(),
            id_to_string: Vec::new(),
        }
    }

    /// Intern a string and return its ID
    /// If the string is already interned, returns the existing ID
    pub fn intern(&mut self, s: &str) -> StringId {
        if let Some(&id) = self.string_to_id.get(s) {
            return id;
        }

        let id = StringId(self.id_to_string.len() as u32);
        self.id_to_string.push(s.to_string());
        self.string_to_id.insert(s.to_string(), id);
        id
    }

    /// Get the string for a given ID
    /// Panics if the ID is invalid
    pub fn resolve(&self, id: StringId) -> &str {
        &self.id_to_string[id.0 as usize]
    }

    /// Get the string for a given ID, if it exists
    pub fn try_resolve(&self, id: StringId) -> Option<&str> {
        self.id_to_string.get(id.0 as usize).map(|s| s.as_str())
    }

    /// Get the number of unique strings interned
    pub fn len(&self) -> usize {
        self.id_to_string.len()
    }

    /// Check if the interner is empty
    pub fn is_empty(&self) -> bool {
        self.id_to_string.is_empty()
    }
}

impl StringId {
    /// Get the raw u32 value of this ID
    pub fn as_u32(self) -> u32 {
        self.0
    }

    /// Create a StringId from a raw u32 value
    /// This is unchecked and doesn't validate the ID exists in the interner
    pub fn from_u32(id: u32) -> Self {
        Self(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_basic() {
        let mut interner = StringInterner::new();

        let id1 = interner.intern("hello");
        let id2 = interner.intern("world");
        let id3 = interner.intern("hello"); // Same as id1

        assert_eq!(id1, id3);
        assert_ne!(id1, id2);

        assert_eq!(interner.resolve(id1), "hello");
        assert_eq!(interner.resolve(id2), "world");
    }

    #[test]
    fn test_intern_deduplication() {
        let mut interner = StringInterner::new();

        // Intern the same string 100 times
        let ids: Vec<_> = (0..100).map(|_| interner.intern("test")).collect();

        // All IDs should be the same
        assert!(ids.iter().all(|&id| id == ids[0]));

        // Should only have one unique string
        assert_eq!(interner.len(), 1);
    }

    #[test]
    fn test_intern_many_unique() {
        let mut interner = StringInterner::new();

        let strings = vec!["foo", "bar", "baz", "qux", "test", "hello", "world"];
        let ids: Vec<_> = strings.iter().map(|s| interner.intern(s)).collect();

        // All IDs should be unique
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j]);
            }
        }

        // Verify we can resolve them all
        for (i, &id) in ids.iter().enumerate() {
            assert_eq!(interner.resolve(id), strings[i]);
        }

        assert_eq!(interner.len(), strings.len());
    }

    #[test]
    fn test_try_resolve() {
        let mut interner = StringInterner::new();

        let id = interner.intern("test");
        assert_eq!(interner.try_resolve(id), Some("test"));

        // Invalid ID
        let invalid_id = StringId::from_u32(9999);
        assert_eq!(interner.try_resolve(invalid_id), None);
    }
}
