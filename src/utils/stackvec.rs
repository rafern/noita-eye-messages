use std::{hint::cold_path, mem::MaybeUninit, ops::{Index, IndexMut}};

pub struct StackVec<T, const MAX_SMALL_LEN: usize> {
    small_len: usize,
    small_data: [MaybeUninit<T>; MAX_SMALL_LEN],
    big_data: Vec<T>,
}

pub struct StackVecIter<'a, T, const MAX_SMALL_LEN: usize> {
    vec: &'a StackVec<T, MAX_SMALL_LEN>,
    head: usize,
    tail: usize,
}

impl<T, const MAX_SMALL_LEN: usize> StackVec<T, MAX_SMALL_LEN> {
    pub const fn new() -> Self {
        Self {
            small_data: [const { MaybeUninit::uninit() }; MAX_SMALL_LEN],
            small_len: 0,
            big_data: Vec::new(),
        }
    }

    #[inline]
    pub const fn len(&self) -> usize {
        let len = self.small_len;
        unsafe { std::hint::assert_unchecked(len < MAX_SMALL_LEN) };
        unsafe { len.unchecked_add(self.big_data.len()) }
    }

    pub const fn iter(&'_ self) -> StackVecIter<'_, T, MAX_SMALL_LEN> {
        StackVecIter {
            vec: self,
            head: 0,
            tail: self.len(),
        }
    }

    pub fn push(&mut self, val: T) {
        if self.small_len < MAX_SMALL_LEN {
            unsafe { self.small_data.get_unchecked_mut(self.small_len) }.write(val);
            self.small_len += 1;
        } else {
            self.big_data.push(val);
        }
    }

    pub fn resize_with<F>(&mut self, new_len: usize, mut f: F)
    where F: FnMut() ->  T
    {
        let new_small_len = new_len.min(MAX_SMALL_LEN);
        if new_small_len > self.small_len {
            for i in self.small_len..new_small_len {
                self.small_data[i].write(f());
            }
        } else if new_small_len < self.small_len {
            for i in new_small_len..self.small_len {
                unsafe { self.small_data[i].assume_init_drop() };
            }
        }

        self.small_len = new_small_len;
        self.big_data.resize_with(new_len.saturating_sub(MAX_SMALL_LEN), f);
    }
}

impl<T, const MAX_SMALL_LEN: usize> Default for StackVec<T, MAX_SMALL_LEN> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone, const MAX_SMALL_LEN: usize> Clone for StackVec<T, MAX_SMALL_LEN> {
    fn clone(&self) -> Self {
        let mut small_data = [const { MaybeUninit::uninit() }; MAX_SMALL_LEN];
        for i in 0..self.small_len {
            small_data[i].write(unsafe { self.small_data[i].assume_init_ref() }.clone());
        }

        Self {
            small_data,
            small_len: self.small_len,
            big_data: self.big_data.clone(),
        }
    }
}

impl<T, const MAX_SMALL_LEN: usize> Drop for StackVec<T, MAX_SMALL_LEN> {
    fn drop(&mut self) {
        for i in 0..self.small_len {
            unsafe { self.small_data[i].assume_init_drop() };
        }
    }
}

impl<T, const MAX_SMALL_LEN: usize> Index<usize> for StackVec<T, MAX_SMALL_LEN> {
    type Output = T;
    #[inline]
    fn index(&self, idx: usize) -> &<Self as Index<usize>>::Output {
        if idx < MAX_SMALL_LEN {
            debug_assert!(idx < self.small_len);
            unsafe { self.small_data.get_unchecked(idx).assume_init_ref() }
        } else {
            cold_path();
            let big_idx = idx - MAX_SMALL_LEN;
            debug_assert!(big_idx < self.big_data.len());
            unsafe { self.big_data.get_unchecked(big_idx) }
        }
    }
}

impl<T, const MAX_SMALL_LEN: usize> IndexMut<usize> for StackVec<T, MAX_SMALL_LEN> {
    #[inline]
    fn index_mut(&mut self, idx: usize) -> &mut <Self as Index<usize>>::Output {
        debug_assert!(idx < self.small_len);
        if idx < MAX_SMALL_LEN {
            debug_assert!(idx < self.small_len);
            unsafe { self.small_data.get_unchecked_mut(idx).assume_init_mut() }
        } else {
            cold_path();
            let big_idx = idx - MAX_SMALL_LEN;
            debug_assert!(big_idx < self.big_data.len());
            unsafe { self.big_data.get_unchecked_mut(big_idx) }
        }
    }
}

impl<'a, T, const MAX_SMALL_LEN: usize> Iterator for StackVecIter<'a, T, MAX_SMALL_LEN> {
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

impl<'a, T, const MAX_SMALL_LEN: usize> DoubleEndedIterator for StackVecIter<'a, T, MAX_SMALL_LEN> {
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