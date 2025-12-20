use std::{mem::MaybeUninit, ops::{Index, IndexMut}};

pub struct StackVec<T, const MAX_LEN: usize> {
    len: usize,
    data: [MaybeUninit<T>; MAX_LEN],
}

pub struct StackVecIter<'a, T, const MAX_LEN: usize> {
    vec: &'a StackVec<T, MAX_LEN>,
    head: usize,
    tail: usize,
}

/// Use this like you would use the ArrayVec crate; basically a stack-allocated
/// vector with a limited size, optimised for cache locality
impl<T, const MAX_LEN: usize> StackVec<T, MAX_LEN> {
    pub fn new() -> Self {
        Self {
            len: 0,
            data: [const { MaybeUninit::uninit() }; MAX_LEN],
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn iter(&'_ self) -> StackVecIter<'_, T, MAX_LEN> {
        StackVecIter {
            vec: self,
            head: 0,
            tail: self.len,
        }
    }

    pub fn push(&mut self, val: T) {
        debug_assert!(self.len < MAX_LEN);
        unsafe { self.data.get_unchecked_mut(self.len) }.write(val);
        self.len += 1;
    }

    pub fn resize_with<F>(&mut self, new_len: usize, mut f: F)
    where F: FnMut() ->  T
    {
        if new_len > self.len {
            for i in self.len..new_len {
                self.data[i].write(f());
            }
        } else if new_len < self.len {
            for i in new_len..self.len {
                unsafe { self.data[i].assume_init_drop() };
            }

            self.len = new_len;
        }
    }
}

impl<T, const MAX_LEN: usize> Default for StackVec<T, MAX_LEN> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone, const MAX_LEN: usize> Clone for StackVec<T, MAX_LEN> {
    fn clone(&self) -> Self {
        let mut clone = Self::new();
        clone.len = self.len;
        for i in 0..self.len {
            clone.data[i].write(unsafe { self.data[i].assume_init_ref() }.clone());
        }

        clone
    }
}

impl<T, const MAX_LEN: usize> Drop for StackVec<T, MAX_LEN> {
    fn drop(&mut self) {
        for i in 0..self.len {
            unsafe { self.data[i].assume_init_drop() };
        }
    }
}

impl<T, const MAX_LEN: usize> Index<usize> for StackVec<T, MAX_LEN> {
    type Output = T;
    fn index(&self, idx: usize) -> &<Self as Index<usize>>::Output {
        debug_assert!(idx < self.len);
        unsafe { self.data.get_unchecked(idx).assume_init_ref() }
    }
}

impl<T, const MAX_LEN: usize> IndexMut<usize> for StackVec<T, MAX_LEN> {
    fn index_mut(&mut self, idx: usize) -> &mut <Self as Index<usize>>::Output {
        debug_assert!(idx < self.len);
        unsafe { self.data.get_unchecked_mut(idx).assume_init_mut() }
    }
}

impl<'a, T, const MAX_LEN: usize> Iterator for StackVecIter<'a, T, MAX_LEN> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.head < self.tail {
            let item = Some(&self.vec[self.head]);
            self.head += 1;
            item
        } else {
            None
        }
    }
}

impl<'a, T, const MAX_LEN: usize> DoubleEndedIterator for StackVecIter<'a, T, MAX_LEN> {
    fn next_back(&mut self) -> Option<<Self as Iterator>::Item> {
        if self.head < self.tail {
            self.tail -= 1;
            let item = Some(&self.vec[self.tail]);
            item
        } else {
            None
        }
    }
}