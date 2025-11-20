use std::ops::{Index, IndexMut};

#[derive(Clone, Copy)]
pub struct StackVec<T, const MAX_LEN: usize> {
    len: usize,
    data: [T; MAX_LEN],
}

pub struct StackVecIter<'a, T, const MAX_LEN: usize> {
    vec: &'a StackVec<T, MAX_LEN>,
    idx: usize,
}

impl<T, const MAX_LEN: usize> StackVec<T, MAX_LEN> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn iter(&'_ self) -> StackVecIter<'_, T, MAX_LEN> {
        StackVecIter {
            vec: self,
            idx: 0,
        }
    }

    pub fn push(&mut self, val: T) {
        debug_assert!(self.len < MAX_LEN);
        self.data[self.len] = val;
        self.len += 1;
    }
}

impl<T, const MAX_LEN: usize> Index<usize> for StackVec<T, MAX_LEN> {
    type Output = T;
    fn index(&self, idx: usize) -> &<Self as Index<usize>>::Output {
        debug_assert!(idx < self.len);
        &self.data[idx]
    }
}

impl<T, const MAX_LEN: usize> IndexMut<usize> for StackVec<T, MAX_LEN> {
    fn index_mut(&mut self, idx: usize) -> &mut <Self as Index<usize>>::Output {
        debug_assert!(idx < self.len);
        &mut self.data[idx]
    }
}

impl<T: Default, const MAX_LEN: usize> Default for StackVec<T, MAX_LEN> {
    fn default() -> Self {
        let mut val = StackVec {
            len: 0,
            data: unsafe { std::mem::zeroed() },
        };

        for i in 0..MAX_LEN {
            val.data[i] = T::default();
        }

        val
    }
}

impl<'a, T, const MAX_LEN: usize> Iterator for StackVecIter<'a, T, MAX_LEN> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx < self.vec.len {
            let item = Some(&self.vec.data[self.idx]);
            self.idx += 1;
            item
        } else {
            None
        }
    }
}
