use std::collections::HashMap;

use cot::router::{Route, RouteKind};
use matchit::{Match, Router as MatchitRouter};

use crate::router::RouteConflictError;
use crate::router::path::{AbsolutePath, PathPart};

pub(super) const NESTED_ROUTER_PARAM: &str = "__cot_nested_router__";

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub(super) struct MatchitPattern(String);

impl MatchitPattern {
    #[must_use]
    pub(super) fn new<T: Into<String>>(pattern: T) -> Self {
        Self(pattern.into())
    }

    #[must_use]
    pub(super) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<MatchitPattern> for String {
    fn from(value: MatchitPattern) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Entry {
    Handler(usize),
    Router(usize),
}

#[derive(Debug, Clone)]
pub(super) struct RouteTrie {
    inner: MatchitRouter<Entry>,
}

impl RouteTrie {
    pub(super) fn build(routes: &[Route]) -> super::Result<Self> {
        let mut inner = MatchitRouter::new();

        let mut pattern_map: HashMap<MatchitPattern, (Option<usize>, Option<usize>)> =
            HashMap::new();
        for (i, route) in routes.iter().enumerate() {
            let pattern = if route.kind() == RouteKind::Router {
                // normalize path of sub-routers since we will attach an internal wildcard
                // sentinel. This should also allow us reject routes for
                // routers(sub-routers) who's version without a trailing slash
                // already exist. (eg. `foo` and `foo/`cannot overlap as sub-routers)
                let url = route.url();
                let trimmed = url
                    .strip_suffix('/')
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&url);
                MatchitPattern::new(trimmed)
            } else {
                MatchitPattern::try_from(route.url.clone())?
            };
            let entry = pattern_map.entry(pattern).or_default();
            match route.kind() {
                RouteKind::Handler => {
                    if let Some(existing) = entry.0 {
                        return Err(RouteConflictError::DuplicateHandler {
                            existing: routes[existing].url(),
                            new: route.url(),
                        }
                        .into());
                    }
                    entry.0 = Some(i);
                }
                RouteKind::Router => {
                    if let Some(existing) = entry.1 {
                        return Err(RouteConflictError::DuplicateRouter {
                            existing: routes[existing].url(),
                            new: route.url(),
                        }
                        .into());
                    }
                    entry.1 = Some(i);
                }
            }
        }

        let mut entries: Vec<_> = pattern_map.into_iter().collect();
        // sort for deterministic insertion behavior
        entries.sort_by_key(|(_, (handler_idx, router_idx))| {
            handler_idx
                .or(*router_idx)
                .expect("route index should exist")
        });

        for (_, (handler_idx, router_idx)) in entries {
            let value = match (handler_idx, router_idx) {
                (Some(h), None) => Entry::Handler(h),
                (None, Some(r)) => Entry::Router(r),
                // for cases where a handler overlaps a router for the same route/path, the handler
                // takes precedence.
                (Some(h), Some(_r)) => Entry::Handler(h),
                (None, None) => unreachable!("there should always be a route or handler or both"),
            };

            let route_idx = handler_idx
                .or(router_idx)
                .expect("route index should exist");

            // we insert the original path, not the (possibly trimmed) deduped route so that
            // routers(sub-routers) that were mounted/declared with trailing slashes still
            // match.
            let insertion_pattern = MatchitPattern::try_from(routes[route_idx].url.clone())?;
            Self::insert_or_diagnose(
                &mut inner,
                insertion_pattern,
                value,
                &routes[route_idx],
                routes,
            )?;

            // when a nested router is provided, we treat it as a "false" wildcard segment
            // and keep a sentinel there so we can use that to find what sub router to
            // search at lookup time.
            if let Some(r) = router_idx {
                let prefix = AbsolutePath::new(routes[route_idx].url());
                let wildcard_suffix = AbsolutePath::new(format!("{{*{NESTED_ROUTER_PARAM}}}"));
                let wildcard = prefix.join(&wildcard_suffix);

                Self::insert_or_diagnose(
                    &mut inner,
                    MatchitPattern::new(wildcard.as_str()),
                    Entry::Router(r),
                    &routes[r],
                    routes,
                )?;
            }
        }

        Ok(Self { inner })
    }

    fn insert_or_diagnose(
        trie: &mut MatchitRouter<Entry>,
        pattern: MatchitPattern,
        value: Entry,
        new_route: &Route,
        routes: &[Route],
    ) -> super::Result<()> {
        trie.insert(pattern, value)
            .map_err(|err| Self::diagnose(new_route, err, routes).into())
    }

    fn diagnose(
        new_route: &Route,
        err: matchit::InsertError,
        routes: &[Route],
    ) -> RouteConflictError {
        match err {
            matchit::InsertError::Conflict { with } => {
                let existing_route = routes.iter().find(|r| {
                    MatchitPattern::try_from(r.url.clone()).is_ok_and(|p| p.as_str() == with)
                });

                if let Some(existing_route) = existing_route {
                    Self::classify(existing_route, new_route)
                } else {
                    RouteConflictError::RouteInsert(matchit::InsertError::Conflict { with })
                }
            }

            other => RouteConflictError::RouteInsert(other),
        }
    }

    fn classify(existing_route: &Route, new_route: &Route) -> RouteConflictError {
        for (existing_part, new_part) in
            existing_route.url.parts().iter().zip(new_route.url.parts())
        {
            match (existing_part, new_part) {
                (PathPart::Param { name: a }, PathPart::Param { name: b }) if a != b => {
                    return RouteConflictError::ConflictingParamName {
                        existing: existing_route.url(),
                        existing_name: a.clone(),
                        new: new_route.url(),
                        new_name: b.clone(),
                    };
                }
                (PathPart::Wildcard { name: a }, PathPart::Wildcard { name: b }) if a != b => {
                    return RouteConflictError::ConflictingWildcardName {
                        existing: existing_route.url(),
                        existing_name: a.clone(),
                        new: new_route.url(),
                        new_name: b.clone(),
                    };
                }
                (PathPart::Wildcard { .. }, PathPart::Wildcard { .. }) => {
                    return RouteConflictError::DuplicateWildcard {
                        existing: existing_route.url(),
                        new: new_route.url(),
                    };
                }
                _ => continue,
            }
        }

        // Every segment matched so this is a duplicate
        RouteConflictError::DuplicateHandler {
            existing: existing_route.url(),
            new: new_route.url(),
        }
    }

    pub(super) fn at<'a>(&'a self, path: &'a str) -> Option<Match<'a, 'a, &'a Entry>> {
        self.inner.at(path).ok()
    }
}

#[cfg(test)]
mod tests {
    use cot::router::Route;

    use super::*;
    use crate::html::Html;
    use crate::router::Router;

    async fn handler() -> Html {
        Html::new("ok")
    }

    fn route(url: &str) -> Route {
        Route::with_handler(url, handler)
    }

    #[test]
    fn build_single_handler_route() {
        let routes = vec![route("/users")];
        let trie = RouteTrie::build(&routes).unwrap();

        let m = trie.at("/users").unwrap();
        assert!(matches!(m.value, Entry::Handler(0)));
    }

    #[test]
    fn build_no_match_returns_none() {
        let routes = vec![route("/users")];
        let trie = RouteTrie::build(&routes).unwrap();

        assert!(trie.at("/other").is_none());
    }

    #[test]
    fn build_root_path_matches() {
        let routes = vec![route("/")];
        let trie = RouteTrie::build(&routes).unwrap();

        assert!(matches!(trie.at("/").unwrap().value, Entry::Handler(0)));
    }

    #[test]
    fn build_param_route_captures_value() {
        let routes = vec![route("/users/{id}")];
        let trie = RouteTrie::build(&routes).unwrap();

        let m = trie.at("/users/42").unwrap();
        assert!(matches!(m.value, Entry::Handler(0)));
        assert_eq!(m.params.get("id"), Some("42"));
    }

    #[test]
    fn build_wildcard_route_captures_remaining_path() {
        let routes = vec![route("/static/{*path}")];
        let trie = RouteTrie::build(&routes).unwrap();

        let m = trie.at("/static/css/app.css").unwrap();
        assert!(matches!(m.value, Entry::Handler(0)));
        assert_eq!(m.params.get("path"), Some("css/app.css"));
    }

    #[test]
    fn build_router_route_inserts_wildcard_sentinel() {
        let sub_router = Router::with_urls(vec![route("/inner")]);
        let routes = vec![Route::with_router("/api", sub_router)];
        let trie = RouteTrie::build(&routes).unwrap();

        assert!(matches!(trie.at("/api").unwrap().value, Entry::Router(0)));

        let m = trie.at("/api/inner").unwrap();
        assert!(matches!(m.value, Entry::Router(0)));
        assert_eq!(m.params.get(NESTED_ROUTER_PARAM), Some("inner"));
    }

    #[test]
    fn build_router_trailing_slash_prefix_does_not_double_slash() {
        let sub_router = Router::with_urls(vec![route("/inner")]);
        let routes = vec![Route::with_router("/api/", sub_router)];
        let trie = RouteTrie::build(&routes).unwrap();

        let m = trie.at("/api/inner").unwrap();
        assert_eq!(m.params.get(NESTED_ROUTER_PARAM), Some("inner"));
    }

    #[test]
    fn build_combined_handler_and_router_same_path() {
        let sub_router = Router::with_urls(vec![route("/inner")]);
        let routes = vec![Route::with_router("/api", sub_router), route("/api")];
        let trie = RouteTrie::build(&routes).unwrap();

        assert!(matches!(trie.at("/api").unwrap().value, Entry::Handler(1)));
    }

    #[test]
    fn static_route_priority_over_param_route() {
        let routes = vec![route("/users/{id}"), route("/users/new")];
        let trie = RouteTrie::build(&routes).unwrap();

        assert!(matches!(
            trie.at("/users/new").unwrap().value,
            Entry::Handler(1)
        ));
    }

    #[test]
    fn build_duplicate_handler_errors() {
        let routes = vec![route("/users"), route("/users")];
        let err = RouteTrie::build(&routes).unwrap_err();
        assert!(err.to_string().contains("duplicate route"));
    }

    #[test]
    fn build_duplicate_router_errors() {
        let routes = vec![
            Route::with_router("/users", Router::empty()),
            Route::with_router("/users", Router::empty()),
        ];
        let err = RouteTrie::build(&routes).unwrap_err();
        assert!(err.to_string().contains("duplicate nested router"));
    }

    #[test]
    fn build_conflicting_param_names_errors() {
        let routes = vec![route("/foo/{bar}/"), route("/foo/{baz}/")];
        let err = RouteTrie::build(&routes).unwrap_err();
        assert!(err.to_string().contains("conflicting route parameters"));
    }

    #[test]
    fn build_conflicting_wildcard_names_errors() {
        let routes = vec![route("/static/{*path}"), route("/static/{*file_path}")];
        let err = RouteTrie::build(&routes).unwrap_err();
        assert!(err.to_string().contains("conflicting wildcard parameters"));
    }

    #[test]
    fn build_duplicate_wildcard_errors() {
        let routes = vec![route("/static/{*path}"), route("/static/{*path}")];
        let err = RouteTrie::build(&routes).unwrap_err();
        assert!(err.to_string().contains("duplicate route"));
    }

    #[test]
    fn build_root_mounted_router_matches_root_path() {
        let sub_router = Router::with_urls(vec![route("/")]);
        let routes = vec![Route::with_router("", sub_router)];
        let trie = RouteTrie::build(&routes).unwrap();

        let m = trie.at("/").unwrap();
        assert!(matches!(m.value, Entry::Router(0)));
    }

    #[test]
    fn build_root_mounted_router_exact_match_has_no_wildcard_capture() {
        let sub_router = Router::with_urls(vec![route("/")]);
        let routes = vec![Route::with_router("/", sub_router)];
        let trie = RouteTrie::build(&routes).unwrap();

        let m = trie.at("/").unwrap();
        assert!(m.params.get(NESTED_ROUTER_PARAM).is_none());
    }

    #[test]
    fn matchit_pattern_new_and_as_str() {
        let pattern = MatchitPattern::new("/users/{id}");
        assert_eq!(pattern.as_str(), "/users/{id}");
    }

    #[test]
    fn matchit_pattern_into_string() {
        let pattern = MatchitPattern::new("/users");
        let s: String = pattern.into();
        assert_eq!(s, "/users");
    }

    #[test]
    fn build_root_mounted_router_pattern_not_trimmed_to_empty() {
        let sub_router = Router::with_urls(vec![route("/inner")]);
        let routes = vec![Route::with_router("/", sub_router)];
        let trie = RouteTrie::build(&routes).unwrap();
        assert!(trie.at("/inner").is_some());
    }

    #[test]
    fn build_router_mount_slash_and_no_slash_variants_conflict_with_clear_error() {
        let router1 = Router::with_urls(vec![route("/foo")]);
        let router2 = Router::with_urls(vec![route("/bar")]);
        let routes = vec![
            Route::with_router("/admin", router1),
            Route::with_router("/admin/", router2),
        ];
        let err = RouteTrie::build(&routes).unwrap_err();
        assert!(err.to_string().contains("duplicate nested router"));
    }
}
