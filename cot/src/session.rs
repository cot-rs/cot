use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]
pub struct Session {
    // tower_sessions::Session internally is two Arcs, so it's cheap to clone
    inner: tower_sessions::Session,
}

impl Session {
    pub(crate) fn new(inner: tower_sessions::Session) -> Self {
        Self { inner }
    }
}

impl Deref for Session {
    type Target = tower_sessions::Session;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
