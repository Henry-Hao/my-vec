use std::alloc;
use std::ops::{Deref, DerefMut};
use std::{marker::PhantomData, ptr::NonNull};

pub struct MyVec<T> {
    /// 1. NonNull<T> will never be Null
    /// 2. NonNull<T> is covariant over T
    ptr: NonNull<T>,
    cap: usize,
    len: usize,
    /// Pretending to own T for dropck later
    _marker: PhantomData<T>,
}

unsafe impl<T: Sync> Sync for MyVec<T> {}
unsafe impl<T: Send> Send for MyVec<T> {}

impl<T> MyVec<T> {
    pub fn new() -> Self {
        assert!(
            std::mem::align_of::<T>() != 0,
            "Zero-Sized-Types are not allowed to create Vec"
        );
        MyVec {
            ptr: NonNull::dangling(),
            cap: 0,
            len: 0,
            _marker: PhantomData,
        }
    }

    fn grow(&mut self) {
        let new_cap = if self.cap == 0 { 1 } else { self.cap * 2 };
        let new_layout = alloc::Layout::array::<T>(new_cap).unwrap();

        // ptr::offset takes an `isize` parameter which is the max number of units of T a pointer
        // can possibly reach
        assert!(
            new_layout.size() <= isize::MAX as usize,
            "Allocation too large"
        );

        let new_ptr = if self.cap == 0 {
            unsafe { alloc::alloc(new_layout) }
        } else {
            let old_layout = alloc::Layout::array::<T>(self.cap).unwrap();
            let old_ptr = self.ptr.as_ptr() as *mut u8;
            unsafe { alloc::realloc(old_ptr, old_layout, new_cap) }
        };

        // if allocation failed, None will be returned
        self.ptr = match NonNull::new(new_ptr as *mut T) {
            Some(p) => p,
            None => {
                alloc::handle_alloc_error(new_layout);
            }
        };
        self.cap = new_cap;
    }

    pub fn push(&mut self, ele: T) {
        if self.len == self.cap {
            self.grow();
        }
        unsafe {
            std::ptr::write(self.ptr.as_ptr().add(self.len), ele);
        }

        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe { Some(std::ptr::read(self.ptr.as_ptr().add(self.len))) }
        }
    }

    pub fn insert(&mut self, idx: usize, ele: T) {
        assert!(idx <= self.len, "index out of bounds");
        if self.len == self.cap {
            self.grow();
        }
        unsafe {
            let ptr = self.ptr.as_ptr().add(idx);
            let new_ptr = self.ptr.as_ptr().add(idx + 1);
            let count = self.len - idx;
            std::ptr::copy(ptr, new_ptr, count);
            std::ptr::write(ptr, ele);
            self.len += 1;
        }
    }

    pub fn remove(&mut self, idx: usize) -> T {
        assert!(idx < self.len, "index out of bounds");
        unsafe {
            let ptr = self.ptr.as_ptr().add(idx + 1);
            let new_ptr = self.ptr.as_ptr().add(idx);
            let item = new_ptr.read();
            let count = self.len - idx - 1;
            std::ptr::copy(ptr, new_ptr, count);
            self.len -= 1;
            return item;
        }
    }

    pub fn into_iter(self) -> IntoIter<T> {
        let ptr = self.ptr;
        let len = self.len;
        let cap = self.cap;

        unsafe {
            // take ownership of self without running its destructor
            std::mem::forget(self);
            IntoIter {
                buf: ptr,
                cap: cap,
                start: ptr.as_ptr(),
                end: if cap == 0 {
                    ptr.as_ptr()
                } else {
                    ptr.as_ptr().add(len)
                },
                _marker: PhantomData,
            }
        }
    }
}

impl<T> Deref for MyVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}

impl<T> DerefMut for MyVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl<T> Drop for MyVec<T> {
    fn drop(&mut self) {
        // if self.cap == 0, nothing has been allocated
        if self.cap != 0 {
            // this could be removed when T:!Drop as in the elements don't need to be dropped
            while let Some(_) = self.pop() {}
            unsafe {
                std::alloc::dealloc(
                    self.ptr.as_ptr() as *mut u8,
                    alloc::Layout::array::<T>(self.cap).unwrap(),
                )
            }
        }
    }
}

pub struct IntoIter<T> {
    buf: NonNull<T>,
    cap: usize,
    start: *const T,
    end: *const T,
    _marker: PhantomData<T>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start == self.end {
            None
        } else {
            self.start = unsafe { self.start.add(1) };
            Some(unsafe { std::ptr::read(self.start.sub(1)) })
        }
    }

}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.start == self.end {
            None
        } else {
            self.end = unsafe { self.end.sub(1) };
            Some(unsafe { std::ptr::read(self.end) })
        }
    }
}

impl<T> Drop for IntoIter<T> {
    fn drop(&mut self) {
        if self.cap != 0 {
            for _ in &mut *self {}
            let layout = alloc::Layout::array::<T>(self.cap).unwrap();
            unsafe {
                alloc::dealloc(self.buf.as_ptr() as *mut u8, layout);
            }

        }
    }
}

#[cfg(test)]
mod tests {
    use super::MyVec;
    #[test]
    fn create_new_success() {
        let v: MyVec<i32> = MyVec::new();
        assert!(std::mem::size_of_val(&v) != 0);
    }
    #[test]
    #[should_panic]
    fn create_new_fail() {
        let v: MyVec<()> = MyVec::new();
        assert!(std::mem::size_of_val(&v) == 0);
    }

    #[test]
    fn push() {
        let mut v: MyVec<i32> = MyVec::new();
        assert_eq!(v.len(), 0);
        v.push(1);
        assert_eq!(v.len(), 1);
        v.push(2);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn pop() {
        let mut v: MyVec<i32> = MyVec::new();
        v.push(1);
        v.push(2);
        assert_eq!(v.pop(), Some(2));
        assert_eq!(v.pop(), Some(1));
        assert_eq!(v.pop(), None);
    }

    #[test]
    fn deref_to_slice_inbound() {
        let mut v: MyVec<i32> = MyVec::new();
        v.push(1);
        v.push(2);
        v.push(3);
        assert_eq!(v[2], 3);
    }

    #[test]
    #[should_panic]
    fn deref_to_slice_outbound() {
        let mut v: MyVec<i32> = MyVec::new();
        v.push(1);
        v.push(2);
        assert_eq!(v[2], 3);
    }

    #[test]
    fn deref_mut_inbound() {
        let mut v: MyVec<i32> = MyVec::new();
        v.push(1);
        v.push(2);
        v.push(3);
        v[1] = 4;
        assert_eq!(v[2], 3);
    }

    #[test]
    #[should_panic]
    fn deref_mut_outbound() {
        let mut v: MyVec<i32> = MyVec::new();
        v.push(1);
        v.push(2);
        v.push(3);
        v[4] = 4;
    }

    #[test]
    fn test_insert() {
        let mut v: MyVec<i32> = MyVec::new();
        v.push(1);
        v.push(2);
        v.push(3);
        v.insert(2, 9);
        assert_eq!(v[3], 3);
        assert_eq!(v[2], 9);
        assert_eq!(v.len(), 4);
    }

    #[test]
    fn test_remove_success() {
        let mut v: MyVec<i32> = MyVec::new();
        v.push(1);
        v.push(2);
        assert_eq!(v.remove(0), 1);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0], 2);
    }

    #[should_panic]
    #[test]
    fn test_remove_fail() {
        let mut v: MyVec<i32> = MyVec::new();
        v.push(1);
        v.push(2);
        v.remove(2);
    }

    #[test]
    fn test_into_iter() {
        let mut v: MyVec<i32> = MyVec::new();
        v.push(1);
        v.push(2);
        v.push(3);
        v.push(4);
        v.push(5);
        let mut it = v.into_iter();
        assert_eq!(it.next(), Some(1));
        assert_eq!(it.next(), Some(2));
        assert_eq!(it.next_back(), Some(5));
        assert_eq!(it.next_back(), Some(4));
        assert_eq!(it.next(), Some(3));
        assert_eq!(it.next(), None);
        assert_eq!(it.next_back(), None);

    }
}
