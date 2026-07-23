use std::any::TypeId;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SliceAccess {
    Shared,
    Exclusive,
}

/// A call-scoped, type-erased shared slice borrow.
///
/// This type is public only so a hidden method on the public
/// `ScriptHostObject` trait can return it. Its module is private, its fields
/// are private, and only `vela_host` can construct or inspect it.
pub struct ErasedSliceRef<'a> {
    data: NonNull<u8>,
    len: usize,
    element_type: TypeId,
    access: SliceAccess,
    _borrow: PhantomData<(&'a [()], Rc<()>)>,
}

impl<'a> ErasedSliceRef<'a> {
    pub(crate) fn new<T: 'static>(value: &'a [T]) -> Self {
        Self {
            data: NonNull::from(value).cast(),
            len: value.len(),
            element_type: TypeId::of::<T>(),
            access: SliceAccess::Shared,
            _borrow: PhantomData,
        }
    }

    pub(crate) fn downcast<T: 'static>(self) -> Option<&'a [T]> {
        if self.element_type != TypeId::of::<T>() || self.access != SliceAccess::Shared {
            return None;
        }

        // SAFETY: `data` came from a live `&'a [T]`, so it is non-null,
        // valid for `len` elements, and aligned for `T`, including empty and
        // zero-sized slices. The checked concrete `TypeId` proves that the
        // reconstructed element type is exactly `T`, and the returned borrow
        // cannot outlive `'a`. The origin was shared, so producing another
        // shared reference preserves aliasing. The `Rc` marker prevents the
        // erased token from being Send or Sync; any later thread transfer of
        // the reconstructed reference is governed by Rust's `T: Sync` rules.
        // No allocation or ownership is transferred, and this code neither
        // drops nor deallocates the pointee.
        Some(unsafe { std::slice::from_raw_parts(self.data.cast::<T>().as_ptr(), self.len) })
    }
}

/// A call-scoped, type-erased exclusive slice borrow.
///
/// This is deliberately neither `Clone` nor `Copy`. The only typed downcast
/// consumes the token, preserving the uniqueness of the originating borrow.
pub struct ErasedSliceMut<'a> {
    data: NonNull<u8>,
    len: usize,
    element_type: TypeId,
    access: SliceAccess,
    _borrow: PhantomData<(&'a mut [()], Rc<()>)>,
}

impl<'a> ErasedSliceMut<'a> {
    pub(crate) fn new<T: 'static>(value: &'a mut [T]) -> Self {
        let len = value.len();
        Self {
            data: NonNull::from(value).cast(),
            len,
            element_type: TypeId::of::<T>(),
            access: SliceAccess::Exclusive,
            _borrow: PhantomData,
        }
    }

    pub(crate) fn downcast<T: 'static>(self) -> Option<&'a mut [T]> {
        if self.element_type != TypeId::of::<T>() || self.access != SliceAccess::Exclusive {
            return None;
        }

        // SAFETY: `data` came from a live, exclusive `&'a mut [T]`, so it is
        // non-null, valid for `len` elements, and aligned for `T`, including
        // empty and zero-sized slices. The checked concrete `TypeId` proves
        // that the reconstructed element type is exactly `T`, and the result
        // cannot outlive `'a`. The token owns the originating exclusive
        // borrow, is neither Clone nor Copy, and is consumed here, so it
        // cannot manufacture simultaneous mutable references. The `Rc`
        // marker prevents the erased token from being Send or Sync; any later
        // thread transfer of the reconstructed reference is governed by
        // Rust's `T: Send` rules. No allocation or ownership is transferred,
        // and this code neither drops nor deallocates the pointee.
        Some(unsafe { std::slice::from_raw_parts_mut(self.data.cast::<T>().as_ptr(), self.len) })
    }
}

#[cfg(test)]
mod tests {
    use super::{ErasedSliceMut, ErasedSliceRef};

    #[derive(Debug, Eq, PartialEq)]
    struct Marker;

    #[test]
    fn shared_downcast_preserves_values_and_rejects_wrong_element_type() {
        let values = [2_i64, 3, 5];
        let erased = ErasedSliceRef::new(&values);
        assert_eq!(erased.downcast::<i64>(), Some(values.as_slice()));

        let erased = ErasedSliceRef::new(&values);
        assert!(erased.downcast::<u64>().is_none());
    }

    #[test]
    fn exclusive_downcast_consumes_token_and_writes_through() {
        let mut values = [2_i64, 3, 5];
        let erased = ErasedSliceMut::new(&mut values);
        let borrowed = erased.downcast::<i64>().expect("matching slice type");
        borrowed[1] = 8;
        assert_eq!(values, [2, 8, 5]);
    }

    #[test]
    fn exclusive_downcast_rejects_wrong_element_type_without_reconstruction() {
        let mut values = [2_i64, 3, 5];
        let erased = ErasedSliceMut::new(&mut values);
        assert!(erased.downcast::<u64>().is_none());
        assert_eq!(values, [2, 3, 5]);
    }

    #[test]
    fn empty_and_zero_sized_slices_retain_their_lengths() {
        let empty: [i64; 0] = [];
        assert_eq!(
            ErasedSliceRef::new(&empty)
                .downcast::<i64>()
                .expect("empty slice type")
                .len(),
            0
        );

        let mut markers = [Marker, Marker, Marker];
        assert_eq!(
            ErasedSliceMut::new(&mut markers)
                .downcast::<Marker>()
                .expect("zero-sized slice type")
                .len(),
            3
        );
    }
}
