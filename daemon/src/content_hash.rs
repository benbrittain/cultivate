//! Portable, stable hashing suitable for identifying values
//!
//! Copied from the Jujutsu source code and modified for blake3.
//! Might give up on keeping daemon a seperate code base?

use itertools::Itertools as _;

/// Portable, stable hashing suitable for identifying values
///
/// Variable-length sequences should hash a 64-bit little-endian representation
/// of their length, then their elements in order. Unordered containers should
/// order their elements according to their `Ord` implementation. Enums should
/// hash a 32-bit little-endian encoding of the ordinal number of the enum
/// variant, then the variant's fields in lexical order.
pub trait ContentHash {
    /// Update the hasher state with this object's content
    fn update(&self, state: &mut blake3::Hasher);
}

/// The 512-bit BLAKE2b content hash
pub fn blake3(x: &(impl ContentHash + ?Sized + std::fmt::Debug)) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    dbg!("{:?}", x);
    x.update(&mut hasher);
    dbg!("hel");
    dbg!(hasher.finalize())
}

impl ContentHash for () {
    fn update(&self, _: &mut blake3::Hasher) {}
}

impl ContentHash for bool {
    fn update(&self, state: &mut blake3::Hasher) {
        u8::from(*self).update(state);
    }
}

impl ContentHash for u8 {
    fn update(&self, state: &mut blake3::Hasher) {
        state.update(&[*self]);
    }
}

impl ContentHash for i32 {
    fn update(&self, state: &mut blake3::Hasher) {
        state.update(&self.to_le_bytes());
    }
}

impl ContentHash for i64 {
    fn update(&self, state: &mut blake3::Hasher) {
        state.update(&self.to_le_bytes());
    }
}

// TODO: Specialize for [u8] once specialization exists
impl<T: ContentHash + std::fmt::Debug> ContentHash for [T] {
    fn update(&self, state: &mut blake3::Hasher) {
        state.update(&(self.len() as u64).to_le_bytes());
        for x in self {
            x.update(state);
        }
    }
}

impl<T: ContentHash + std::fmt::Debug, V: ContentHash + std::fmt::Debug> ContentHash for (T, V) {
    fn update(&self, state: &mut blake3::Hasher) {
        dbg!("{:?}", self);
        self.0.update(state);
        self.1.update(state);
    }
}

impl<T: ContentHash + std::fmt::Debug> ContentHash for Vec<T> {
    fn update(&self, state: &mut blake3::Hasher) {
        dbg!("{:?}", self);
        self.as_slice().update(state)
    }
}

impl ContentHash for String {
    fn update(&self, state: &mut blake3::Hasher) {
        dbg!(&self);
        self.as_bytes().update(state);
    }
}

impl<T: ContentHash + std::fmt::Debug> ContentHash for Option<T> {
    fn update(&self, state: &mut blake3::Hasher) {
        match self {
            None => {
                state.update(&[0]);
            }
            Some(x) => {
                state.update(&[1]);
                x.update(state);
            }
        }
    }
}

impl<K, V> ContentHash for std::collections::HashMap<K, V>
where
    K: ContentHash + Ord,
    V: ContentHash,
{
    fn update(&self, state: &mut blake3::Hasher) {
        state.update(&(self.len() as u64).to_le_bytes());
        let mut kv = self.iter().collect_vec();
        kv.sort_unstable_by_key(|&(k, _)| k);
        for (k, v) in kv {
            k.update(state);
            v.update(state);
        }
    }
}

impl<K> ContentHash for std::collections::HashSet<K>
where
    K: ContentHash + Ord,
{
    fn update(&self, state: &mut blake3::Hasher) {
        state.update(&(self.len() as u64).to_le_bytes());
        for k in self.iter().sorted() {
            k.update(state);
        }
    }
}

impl<K, V> ContentHash for std::collections::BTreeMap<K, V>
where
    K: ContentHash,
    V: ContentHash,
{
    fn update(&self, state: &mut blake3::Hasher) {
        state.update(&(self.len() as u64).to_le_bytes());
        for (k, v) in self.iter() {
            k.update(state);
            v.update(state);
        }
    }
}

macro_rules! content_hash {
    ($(#[$meta:meta])* $vis:vis struct $name:ident {
        $($(#[$field_meta:meta])* $field_vis:vis $field:ident : $ty:ty),* $(,)?
    }) => {
        $(#[$meta])*
        $vis struct $name {
            $($(#[$field_meta])* $field_vis $field : $ty),*
        }

        impl crate::content_hash::ContentHash for $name {
            fn update(&self, state: &mut blake3::Hasher) {
                dbg!(&self);
                $(<$ty as crate::content_hash::ContentHash>::update(&self.$field, state);)*
            }
        }
    };
    ($(#[$meta:meta])* $vis:vis struct $name:ident($field_vis:vis $ty:ty);) => {
        $(#[$meta])*
        $vis struct $name($field_vis $ty);

        impl crate::content_hash::ContentHash for $name {
            fn update(&self, state: &mut blake3::Hasher) {
                dbg!(&self);
                <$ty as crate::content_hash::ContentHash>::update(&self.0, state);
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use super::*;

    #[test]
    fn test_string_sanity() {
        let a = "a".to_string();
        let b = "b".to_string();
        assert_eq!(hash(&a), hash(&a.clone()));
        assert_ne!(hash(&a), hash(&b));
        assert_ne!(hash(&"a".to_string()), hash(&"a\0".to_string()));
    }

    #[test]
    fn test_tuple_sanity() {
        let a = ("a".to_string(), "b".to_string());
        let b = ("b".to_string(), "a".to_string());
        let c = ("b".to_string(), 3);
        assert_eq!(hash(&a), hash(&a.clone()));
        assert_ne!(hash(&b), hash(&a));
    }

    #[test]
    fn test_hash_map_key_value_distinction() {
        let a = [("ab".to_string(), "cd".to_string())]
            .into_iter()
            .collect::<HashMap<_, _>>();
        let b = [("a".to_string(), "bcd".to_string())]
            .into_iter()
            .collect::<HashMap<_, _>>();

        assert_ne!(hash(&a), hash(&b));
    }

    #[test]
    fn test_btree_map_key_value_distinction() {
        let a = [("ab".to_string(), "cd".to_string())]
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let b = [("a".to_string(), "bcd".to_string())]
            .into_iter()
            .collect::<BTreeMap<_, _>>();

        assert_ne!(hash(&a), hash(&b));
    }

    #[test]
    fn test_struct_sanity() {
        content_hash! {
            #[derive(Debug)]
            struct Foo { x: i32 }
        }
        assert_ne!(hash(&Foo { x: 42 }), hash(&Foo { x: 12 }));
    }
    #[test]
    fn test_empty_struct_sanity() {
        content_hash! {
            #[derive(Debug)]
            struct EmptyFoo { }
        }
        assert_eq!(hash(&EmptyFoo {}), hash(&EmptyFoo {}));
    }

    #[test]
    fn test_option_sanity() {
        assert_ne!(hash(&Some(42)), hash(&42));
        assert_ne!(hash(&None::<i32>), hash(&42i32));
    }

    #[test]
    fn test_slice_sanity() {
        assert_ne!(hash(&[42i32][..]), hash(&[12i32][..]));
        assert_ne!(hash(&([] as [i32; 0])[..]), hash(&[42i32][..]));
        assert_ne!(hash(&([] as [i32; 0])[..]), hash(&()));
        assert_ne!(hash(&42i32), hash(&[42i32][..]));
    }

    #[test]
    fn test_consistent_hashing() {
        content_hash! {
            #[derive(Debug)]
            struct Foo { x: Vec<Option<i32>>, y: i64 }
        }
        insta::assert_snapshot!(
            hex::encode(hash(&Foo {
                x: vec![None, Some(42)],
                y: 17
            })),
            @"0b96f17e2aeed714583d62bca1898d577ebc2eff5d15fa03feb8de2785632aa0"
        );
    }

    fn hash(x: &(impl ContentHash + ?Sized + std::fmt::Debug)) -> Vec<u8> {
        blake3(x).as_bytes().to_vec()
    }
}
