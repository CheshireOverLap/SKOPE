use std::ops::{Deref, DerefMut};

/// Mutable reference wrapper returned by queries and `World::get_mut`.
///
/// Mirrors bevy_ecs `Mut<T>` — provides `Deref`/`DerefMut` access.
pub struct Mut<'a, T: ?Sized> {
    pub(crate) value: &'a mut T,
}

impl<T: ?Sized> Deref for Mut<'_, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        self.value
    }
}

impl<T: ?Sized> DerefMut for Mut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}

impl<T: ?Sized + std::fmt::Debug> std::fmt::Debug for Mut<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}
