//! The path-segment trie used for route matching.
//!
//! This is a radix tree keyed on `/`-delimited path segments — explicitly not a
//! linear scan. Each node holds exact-match static children, at most one named
//! parameter child (`:name`), and at most one catch-all wildcard child
//! (`*name`). Matching tries them in priority order **static → param →
//! wildcard**, with backtracking, so lookup cost is proportional to the number
//! of path segments, independent of how many routes are registered.

use std::collections::HashMap;

use crate::error::RouteError;

/// A node in the segment trie, generic over the value stored at terminals.
pub(crate) struct Node<T> {
    /// Exact-match children keyed by segment text.
    statics: HashMap<String, Node<T>>,
    /// Optional `:name` child matching any single non-empty segment.
    param: Option<(String, Box<Node<T>>)>,
    /// Optional `*name` child matching all remaining segments (terminal).
    wildcard: Option<(String, Box<Node<T>>)>,
    /// The value registered at this exact path, if any.
    value: Option<T>,
}

impl<T> Default for Node<T> {
    fn default() -> Self {
        Node {
            statics: HashMap::new(),
            param: None,
            wildcard: None,
            value: None,
        }
    }
}

/// Split a path into its segments, treating a lone `/` as the empty (root) path.
///
/// A trailing slash produces a trailing empty segment, which is matched
/// literally — `/foo` and `/foo/` are therefore distinct paths.
fn split_segments(path: &str) -> Vec<&str> {
    let trimmed = path.strip_prefix('/').unwrap_or(path);
    if trimmed.is_empty() {
        Vec::new()
    } else {
        trimmed.split('/').collect()
    }
}

impl<T> Node<T> {
    pub(crate) fn new() -> Self {
        Node::default()
    }

    /// Navigate to (creating as needed) the value slot for `path`, returning a
    /// mutable reference so the caller can insert or merge a value.
    ///
    /// # Errors
    ///
    /// Returns [`RouteError`] for invalid patterns (empty or misplaced
    /// parameter/wildcard names) or conflicting parameter/wildcard names at the
    /// same position.
    pub(crate) fn at_or_insert(&mut self, path: &str) -> Result<&mut Option<T>, RouteError> {
        let segments = split_segments(path);
        let mut node = self;

        let mut idx = 0;
        while idx < segments.len() {
            let seg = segments[idx];
            let is_last = idx + 1 == segments.len();

            if let Some(name) = seg.strip_prefix(':') {
                if name.is_empty() {
                    return Err(RouteError::invalid_pattern(path, "empty parameter name"));
                }
                if node.param.is_none() {
                    node.param = Some((name.to_owned(), Box::new(Node::new())));
                }
                if let Some((existing, _)) = node.param.as_ref()
                    && existing != name
                {
                    return Err(RouteError::conflict(
                        path,
                        format!(
                            "conflicting parameter names `:{existing}` and `:{name}` at the same position"
                        ),
                    ));
                }
                node = &mut node.param.as_mut().expect("param just ensured").1;
                idx += 1;
            } else if let Some(name) = seg.strip_prefix('*') {
                if !is_last {
                    return Err(RouteError::invalid_pattern(
                        path,
                        "wildcard `*` must be the final path segment",
                    ));
                }
                if name.is_empty() {
                    return Err(RouteError::invalid_pattern(path, "empty wildcard name"));
                }
                match node.wildcard.as_ref() {
                    Some((existing, _)) if existing != name => {
                        return Err(RouteError::conflict(
                            path,
                            format!("conflicting wildcard names `*{existing}` and `*{name}`"),
                        ));
                    }
                    Some(_) => {}
                    None => {
                        node.wildcard = Some((name.to_owned(), Box::new(Node::new())));
                    }
                }
                let wc = node.wildcard.as_mut().expect("wildcard just ensured");
                return Ok(&mut wc.1.value);
            } else {
                node = node.statics.entry(seg.to_owned()).or_default();
                idx += 1;
            }
        }

        Ok(&mut node.value)
    }

    /// Match `path`, returning the terminal value and filling `params` with any
    /// captured parameters (cleared on entry).
    pub(crate) fn match_path<'a>(
        &'a self,
        path: &str,
        params: &mut Vec<(String, String)>,
    ) -> Option<&'a T> {
        params.clear();
        let segments = split_segments(path);
        match_node(self, &segments, params)
    }
}

fn match_node<'a, T>(
    node: &'a Node<T>,
    segments: &[&str],
    params: &mut Vec<(String, String)>,
) -> Option<&'a T> {
    let Some((&seg, rest)) = segments.split_first() else {
        return node.value.as_ref();
    };

    // 1. Static segments take priority.
    if let Some(child) = node.statics.get(seg) {
        let mark = params.len();
        if let Some(value) = match_node(child, rest, params) {
            return Some(value);
        }
        params.truncate(mark);
    }

    // 2. A named parameter matches any single non-empty segment.
    if !seg.is_empty()
        && let Some((name, child)) = &node.param
    {
        let mark = params.len();
        params.push((name.clone(), seg.to_owned()));
        if let Some(value) = match_node(child, rest, params) {
            return Some(value);
        }
        params.truncate(mark);
    }

    // 3. A wildcard captures the remaining path and terminates the match.
    if let Some((name, child)) = &node.wildcard {
        params.push((name.clone(), segments.join("/")));
        return child.value.as_ref();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params_map(params: &[(String, String)]) -> Vec<(&str, &str)> {
        params
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }

    #[test]
    fn root_and_static_routes() {
        let mut tree: Node<&str> = Node::new();
        *tree.at_or_insert("/").unwrap() = Some("root");
        *tree.at_or_insert("/users").unwrap() = Some("users");
        *tree.at_or_insert("/users/active").unwrap() = Some("active");

        let mut p = Vec::new();
        assert_eq!(tree.match_path("/", &mut p), Some(&"root"));
        assert_eq!(tree.match_path("/users", &mut p), Some(&"users"));
        assert_eq!(tree.match_path("/users/active", &mut p), Some(&"active"));
        assert_eq!(tree.match_path("/missing", &mut p), None);
    }

    #[test]
    fn param_capture() {
        // Parameter names must be consistent at the same position (like
        // httprouter / matchit); use `:id` in both routes.
        let mut tree: Node<&str> = Node::new();
        *tree.at_or_insert("/users/:id").unwrap() = Some("user");
        *tree.at_or_insert("/users/:id/orders/:oid").unwrap() = Some("order");

        let mut p = Vec::new();
        assert_eq!(tree.match_path("/users/42", &mut p), Some(&"user"));
        assert_eq!(params_map(&p), vec![("id", "42")]);

        assert_eq!(tree.match_path("/users/7/orders/9", &mut p), Some(&"order"));
        assert_eq!(params_map(&p), vec![("id", "7"), ("oid", "9")]);
    }

    #[test]
    fn priority_static_beats_param_beats_wildcard() {
        let mut tree: Node<&str> = Node::new();
        *tree.at_or_insert("/users/:id").unwrap() = Some("param");
        *tree.at_or_insert("/users/me").unwrap() = Some("static");
        *tree.at_or_insert("/users/*rest").unwrap() = Some("wild");

        let mut p = Vec::new();
        assert_eq!(tree.match_path("/users/me", &mut p), Some(&"static"));
        assert!(p.is_empty());
        assert_eq!(tree.match_path("/users/42", &mut p), Some(&"param"));
        assert_eq!(params_map(&p), vec![("id", "42")]);
        assert_eq!(tree.match_path("/users/a/b/c", &mut p), Some(&"wild"));
        assert_eq!(params_map(&p), vec![("rest", "a/b/c")]);
    }

    #[test]
    fn wildcard_captures_remaining_path() {
        let mut tree: Node<&str> = Node::new();
        *tree.at_or_insert("/assets/*path").unwrap() = Some("assets");

        let mut p = Vec::new();
        assert_eq!(
            tree.match_path("/assets/css/app.css", &mut p),
            Some(&"assets")
        );
        assert_eq!(params_map(&p), vec![("path", "css/app.css")]);
    }

    #[test]
    fn backtracks_from_param_to_wildcard() {
        // `/files/:name/meta` and `/files/*rest`: `/files/a/b` should fall back
        // to the wildcard because the param branch has no `/meta` under `a`.
        let mut tree: Node<&str> = Node::new();
        *tree.at_or_insert("/files/:name/meta").unwrap() = Some("meta");
        *tree.at_or_insert("/files/*rest").unwrap() = Some("wild");

        let mut p = Vec::new();
        assert_eq!(tree.match_path("/files/report/meta", &mut p), Some(&"meta"));
        assert_eq!(params_map(&p), vec![("name", "report")]);
        assert_eq!(tree.match_path("/files/a/b", &mut p), Some(&"wild"));
        assert_eq!(params_map(&p), vec![("rest", "a/b")]);
    }

    #[test]
    fn trailing_slash_is_distinct() {
        let mut tree: Node<&str> = Node::new();
        *tree.at_or_insert("/foo").unwrap() = Some("foo");

        let mut p = Vec::new();
        assert_eq!(tree.match_path("/foo", &mut p), Some(&"foo"));
        assert_eq!(tree.match_path("/foo/", &mut p), None);
    }

    #[test]
    fn conflicting_param_names_error() {
        let mut tree: Node<&str> = Node::new();
        *tree.at_or_insert("/users/:id").unwrap() = Some("a");
        let err = tree.at_or_insert("/users/:name/x").unwrap_err();
        assert!(matches!(err, RouteError::Conflict { .. }));
    }

    #[test]
    fn wildcard_not_last_errors() {
        let mut tree: Node<&str> = Node::new();
        let err = tree.at_or_insert("/a/*rest/b").unwrap_err();
        assert!(matches!(err, RouteError::InvalidPattern { .. }));
    }

    #[test]
    fn empty_param_name_errors() {
        let mut tree: Node<&str> = Node::new();
        let err = tree.at_or_insert("/a/:").unwrap_err();
        assert!(matches!(err, RouteError::InvalidPattern { .. }));
    }
}
