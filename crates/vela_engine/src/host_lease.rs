use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use vela_host::error::HostResult;
use vela_host::lease::{ErasedHostLease, host_lease_unsupported};
use vela_host::path::HostRef;

/// A Rust-only shared lease over one direct host binding.
///
/// ```compile_fail
/// use std::cell::Cell;
/// use vela_engine::host_lease::HostLeaseRef;
///
/// fn requires_sync(_: Option<HostLeaseRef<'static, Cell<i64>>>) {}
/// ```
pub struct HostLeaseRef<'host, T>
where
    T: Sync + 'static,
{
    inner: ErasedHostLease<'host>,
    marker: PhantomData<&'host T>,
}

impl<'host, T> HostLeaseRef<'host, T>
where
    T: Sync + 'static,
{
    #[doc(hidden)]
    pub fn from_erased(inner: ErasedHostLease<'host>, root: HostRef) -> HostResult<Self> {
        let object = inner.object();
        let concrete_type_id = object.host_type_id();
        let matches = object.lease_any().is_some_and(|object| object.is::<T>());
        if !matches || !host_object_type_matches_root(concrete_type_id, root) {
            return Err(host_lease_unsupported(root));
        }
        Ok(Self {
            inner,
            marker: PhantomData,
        })
    }
}

impl<T> Deref for HostLeaseRef<'_, T>
where
    T: Sync + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner
            .object()
            .lease_any()
            .and_then(|object| object.downcast_ref::<T>())
            .expect("HostLeaseRef validates its concrete type at construction")
    }
}

/// A Rust-only exclusive lease over one direct host binding.
///
/// ```compile_fail
/// use std::rc::Rc;
/// use vela_engine::host_lease::HostLeaseMut;
///
/// fn requires_send(_: Option<HostLeaseMut<'static, Rc<i64>>>) {}
/// ```
pub struct HostLeaseMut<'host, T>
where
    T: Send + 'static,
{
    inner: ErasedHostLease<'host>,
    marker: PhantomData<&'host mut T>,
}

impl<'host, T> HostLeaseMut<'host, T>
where
    T: Send + 'static,
{
    #[doc(hidden)]
    pub fn from_erased(inner: ErasedHostLease<'host>, root: HostRef) -> HostResult<Self> {
        let concrete_type_id = inner.object().host_type_id();
        if !inner.is_exclusive()
            || inner
                .object()
                .lease_any()
                .is_none_or(|object| !object.is::<T>())
            || !host_object_type_matches_root(concrete_type_id, root)
        {
            return Err(host_lease_unsupported(root));
        }
        Ok(Self {
            inner,
            marker: PhantomData,
        })
    }
}

fn host_object_type_matches_root(concrete_type_id: vela_common::HostTypeId, root: HostRef) -> bool {
    // Generic standard host fields report zero because vela_host has no type
    // registry dependency; their exact root identity was already preflighted.
    concrete_type_id.get() == 0 || concrete_type_id == root.type_id
}

impl<T> Deref for HostLeaseMut<'_, T>
where
    T: Send + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner
            .object()
            .lease_any()
            .and_then(|object| object.downcast_ref::<T>())
            .expect("HostLeaseMut validates its concrete type at construction")
    }
}

impl<T> DerefMut for HostLeaseMut<'_, T>
where
    T: Send + 'static,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
            .object_mut()
            .and_then(|object| object.lease_any_mut())
            .and_then(|object| object.downcast_mut::<T>())
            .expect("HostLeaseMut owns an exclusive concrete object")
    }
}
