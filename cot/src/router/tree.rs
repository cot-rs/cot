use std::collections::HashMap;

use cot::router::{Route, RouteKind};
use matchit::{Match, Router as MatchitRouter};

use crate::router::RouteConflictError;
use crate::router::path::PathPart;

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
    Combined { handler: usize, _router: usize },
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
            let pattern = MatchitPattern::try_from(route.url.clone())?;
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

        for (pattern, (handler_idx, router_idx)) in pattern_map {
            let value = match (handler_idx, router_idx) {
                (Some(h), None) => Entry::Handler(h),
                (None, Some(r)) => Entry::Router(r),
                (Some(h), Some(r)) => Entry::Combined {
                    handler: h,
                    _router: r,
                },
                (None, None) => unreachable!("there should always be a route or handler or both"),
            };

            let route_idx = handler_idx
                .or(router_idx)
                .expect("route index should exist");
            Self::insert_or_diagnose(
                &mut inner,
                pattern.clone(),
                value,
                &routes[route_idx],
                routes,
            )?;

            // when a nested router is provided, we treat it as a "false" wildcard segment
            // and keep a sentinel there so we can use that to find what sub router to
            // search at lookup time.
            if let Some(r) = router_idx {
                let wildcard = format!(
                    "{}/{{*{NESTED_ROUTER_PARAM}}}",
                    pattern.as_str().trim_end_matches('/')
                );
                Self::insert_or_diagnose(
                    &mut inner,
                    MatchitPattern::new(wildcard),
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
