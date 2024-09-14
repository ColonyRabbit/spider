use crate::CaseInsensitiveString;
use hashbrown::HashSet;
use std::hash::Hash;
use std::marker::PhantomData;
use string_interner::backend::StringBackend;
use string_interner::symbol::SymbolU32;
use string_interner::{DefaultSymbol, StringInterner};

/// The links visited bucket store.
#[derive(Debug, Clone)]
pub struct ListBucket<K = CaseInsensitiveString>
where
    K: Eq + Hash + AsRef<str>,
{
    /// The links visited.
    pub(crate) links_visited: HashSet<DefaultSymbol>,
    /// The string interner.
    pub(crate) interner: StringInterner<StringBackend<DefaultSymbol>>,
    /// Phantom data to link the generic type.
    _marker: PhantomData<K>,
}

impl<K> Default for ListBucket<K>
where
    K: Eq + Hash + AsRef<str>,
{
    fn default() -> Self {
        Self {
            links_visited: HashSet::new(),
            interner: StringInterner::default(),
            _marker: PhantomData,
        }
    }
}

impl<K> ListBucket<K>
where
    K: Eq + Hash + AsRef<str>,
{
    /// New list bucket.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new link to the bucket.
    pub fn insert(&mut self, link: K) {
        let symbol = self.interner.get_or_intern(link.as_ref());
        self.links_visited.insert(symbol);
    }

    /// Does the bucket contain the link.
    pub fn contains(&self, link: &K) -> bool {
        if let Some(symbol) = self.interner.get(link.as_ref()) {
            self.links_visited.contains(&symbol)
        } else {
            false
        }
    }

    /// The bucket length.
    pub fn len(&self) -> usize {
        self.links_visited.len()
    }

    /// Drain the bucket.
    pub fn drain(&mut self) -> hashbrown::hash_set::Drain<'_, SymbolU32> {
        self.links_visited.drain()
    }

    /// Clear the bucket.
    pub fn clear(&mut self) {
        self.links_visited.clear()
    }

    /// Get a vector of all the inner values of the links in the bucket.
    pub fn get_links(&self) -> HashSet<K>
    where
        K: Hash + Clone + From<String>,
    {
        self.links_visited
            .iter()
            .filter_map(|symbol| self.interner.resolve(*symbol))
            .map(|s| K::from(s.to_owned()))
            .collect()
    }

    /// Extend with current links.
    pub fn extend_links(&mut self, links: &mut HashSet<K>, msg: HashSet<K>)
    where
        K: Clone,
    {
        for link in msg {
            let symbol = self.interner.get_or_intern(link.as_ref());
            if !self.links_visited.contains(&symbol) {
                links.insert(link);
            }
        }
    }

    /// Extend with new links.
    pub fn extend_with_new_links(&mut self, links: &mut HashSet<K>, s: K)
    where
        K: Clone,
    {
        if let Some(symbol) = self.interner.get(s.as_ref()) {
            if !self.links_visited.contains(&symbol) {
                links.insert(s);
            }
        } else {
            links.insert(s);
        }
    }
}
