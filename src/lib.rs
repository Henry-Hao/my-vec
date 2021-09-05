use std::{marker::PhantomData, ptr::NonNull};
use std::alloc;

pub struct MyVec<T> {
    /// 1. NonNull<T> will never be Null
    /// 2. NonNull<T> is covariant over T
    ptr: NonNull<T>,
    cap: usize,
    len: usize,
    /// Pretending to own T for dropck later
    _marker: PhantomData<T>
}

unsafe impl <T: Sync>Sync for MyVec<T>{}
unsafe impl <T: Send>Send for MyVec<T>{}

impl<T> MyVec<T> {
    pub fn new() -> Self {
        assert!(std::mem::align_of::<T>() != 0, "Zero-Sized-Types are not allowed to create Vec");
        MyVec {
            ptr: NonNull::dangling(),
            cap: 0,
            len: 0,
            _marker: PhantomData
        }
    }

    fn grow(&mut self) {
        let new_cap = if self.cap == 0 { 1 } else { self.cap * 2 };
        let new_layout = alloc::Layout::array::<T>(new_cap).unwrap();

        // ptr::offset takes an `isize` parameter which is the max number of units of T a pointer
        // can possibly reach
        assert!(new_layout.size() <= isize::MAX as usize, "Allocation too large");

        let new_ptr = if self.cap == 0 {
            unsafe {
                alloc::alloc(new_layout)
            }
        } else {
            let old_layout = alloc::Layout::array::<T>(self.cap).unwrap();
            let old_ptr = self.ptr.as_ptr() as *mut u8;
            unsafe {
                alloc::realloc(old_ptr, old_layout, new_cap)
            }
        };

        // if allocation failed, None will be returned
        self.ptr = match NonNull::new(new_ptr as *mut T){
            Some(p) => p,
            None => {
                alloc::handle_alloc_error(new_layout);
            }
        };
        self.cap = new_cap;
    }
}


#[cfg(test)]
mod tests {
    use super::MyVec;
    #[test]
    fn create_new_success() {
        let v:MyVec<i32> = MyVec::new();
        assert!(std::mem::size_of_val(&v) != 0);
    }
    #[test]
    #[should_panic]
    fn create_new_fail() {
        let v:MyVec<()> = MyVec::new();
        assert!(std::mem::size_of_val(&v) == 0);
    }
}
