//! Captured path parameters.

/// The path parameters captured while matching a request against a route.
///
/// The router inserts a `Params` value into each matched request's extensions,
/// so extractors (from later phases) and handlers can read named segments such
/// as the `id` in `/users/:id`.
///
/// Order reflects the order parameters appear in the matched route.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Params(Vec<(String, String)>);

impl Params {
    /// Create an empty parameter set.
    #[must_use]
    pub fn new() -> Self {
        Params(Vec::new())
    }

    pub(crate) fn from_pairs(pairs: Vec<(String, String)>) -> Self {
        Params(pairs)
    }

    /// Look up the first value captured for `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Return the number of captured parameters.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return `true` if no parameters were captured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over the captured `(name, value)` pairs in match order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }
}

impl<'a> IntoIterator for &'a Params {
    type Item = (&'a str, &'a str);
    type IntoIter = Box<dyn Iterator<Item = (&'a str, &'a str)> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}
