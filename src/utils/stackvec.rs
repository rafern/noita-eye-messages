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
    pub const fn new() -> Self {
        Self {
            len: 0,
            data: [const { MaybeUninit::uninit() }; MAX_LEN],
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn iter(&'_ self) -> StackVecIter<'_, T, MAX_LEN> {
        StackVecIter {
            vec: self,
            head: 0,
            tail: self.len,
        }
    }

    pub const fn push(&mut self, val: T) {
        self.data[self.len].write(val);
        self.len += 1;
    }

    #[inline(always)]
    pub unsafe fn get_unchecked(&self, idx: usize) -> &T {
        unsafe { self.data.get_unchecked(idx).assume_init_ref() }
    }

    #[inline(always)]
    pub unsafe fn get_unchecked_mut(&mut self, idx: usize) -> &mut T {
        unsafe { self.data.get_unchecked_mut(idx).assume_init_mut() }
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
                // SAFETY: ranges from 0..self.len are guaranteed to be initialised
                unsafe { self.data.get_unchecked_mut(i).assume_init_drop() };
            }
        }

        self.len = new_len;
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
            // SAFETY: ranges from 0..self.len are guaranteed to be initialised
            clone.data[i].write(unsafe { self.data[i].assume_init_ref() }.clone());
        }

        clone
    }
}

impl<T, const MAX_LEN: usize> Drop for StackVec<T, MAX_LEN> {
    fn drop(&mut self) {
        for i in 0..self.len {
            // SAFETY: ranges from 0..self.len are guaranteed to be initialised
            unsafe { self.data[i].assume_init_drop() };
        }
    }
}

impl<T, const MAX_LEN: usize> Index<usize> for StackVec<T, MAX_LEN> {
    type Output = T;
    fn index(&self, idx: usize) -> &<Self as Index<usize>>::Output {
        if idx >= self.len {
            panic!("index idx (is {idx}) should be < len (is {})", self.len);
        }

        // SAFETY: ranges from 0..self.len are guaranteed to be initialised
        unsafe { self.data[idx].assume_init_ref() }
    }
}

impl<T, const MAX_LEN: usize> IndexMut<usize> for StackVec<T, MAX_LEN> {
    fn index_mut(&mut self, idx: usize) -> &mut <Self as Index<usize>>::Output {
        if idx >= self.len {
            panic!("index_mut idx (is {idx}) should be < len (is {})", self.len);
        }

        // SAFETY: ranges from 0..self.len are guaranteed to be initialised
        unsafe { self.data[idx].assume_init_mut() }
    }
}

impl<'a, T, const MAX_LEN: usize> Iterator for StackVecIter<'a, T, MAX_LEN> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.head < self.tail {
            // SAFETY: ranges from 0..self.len are guaranteed to be initialised,
            //         self.len cannot be mutated while iterating, and both
            //         self.tail and self.head are guaranteed to be in the range
            //         0..self.len
            let item = Some(unsafe { self.vec.get_unchecked(self.head) });
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
            // SAFETY: ranges from 0..self.len are guaranteed to be initialised,
            //         self.len cannot be mutated while iterating, and both
            //         self.tail and self.head are guaranteed to be in the range
            //         0..self.len
            let item = Some(unsafe { self.vec.get_unchecked(self.tail) });
            item
        } else {
            None
        }
    }
}