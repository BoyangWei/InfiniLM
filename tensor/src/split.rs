﻿use crate::{idim, pattern::Pattern, udim, Affine, Shape, Tensor};
use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
    rc::Rc,
    slice::from_raw_parts_mut,
};

pub trait Splitable {
    fn split(&self) -> Self;
}

impl<T: Clone> Splitable for T {
    #[inline]
    fn split(&self) -> Self {
        self.clone()
    }
}

#[repr(transparent)]
pub struct LocalSplitable<T>(Rc<T>);

impl<T> From<T> for LocalSplitable<T> {
    #[inline]
    fn from(t: T) -> Self {
        Self(Rc::new(t))
    }
}

impl<T> Splitable for LocalSplitable<T> {
    #[inline]
    fn split(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Deref> Deref for LocalSplitable<T> {
    type Target = T::Target;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<T, U> DerefMut for LocalSplitable<T>
where
    T: DerefMut<Target = [U]>,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        let data = self.0.as_ptr().cast_mut();
        let len = self.0.len();
        unsafe { from_raw_parts_mut(data, len) }
    }
}

impl<Physical: Splitable> Tensor<Physical> {
    pub fn split(&self, axis: usize, segments: &[udim]) -> VecDeque<Self> {
        build(axis, segments, &self.shape)
            .into_iter()
            .map(|(shape, affine)| Self {
                layout: self.layout,
                shape,
                pattern: Pattern(affine * &self.pattern.0),
                physical: self.physical.split(),
            })
            .collect()
    }
}

fn build(axis: usize, segments: &[udim], input: &[udim]) -> Vec<(Shape, Affine)> {
    assert!(input.len() > axis);
    assert!(input[axis] >= segments.iter().sum());

    segments
        .iter()
        .scan(0, |prefix, &seg| {
            let shape = input
                .iter()
                .enumerate()
                .map(|(i, &dim)| if i == axis { seg } else { dim })
                .collect();
            let n = input.len();
            let affine = Affine::from_fn(n + 1, n + 1, |r, c| {
                if r == c {
                    1
                } else if r == n {
                    if c == axis {
                        *prefix
                    } else {
                        0
                    }
                } else {
                    0
                }
            });
            *prefix += seg as idim;
            Some((shape, affine))
        })
        .collect()
}

#[test]
fn test() {
    let ans = build(1, &[3, 4, 5], &[11, 12, 13]);
    assert_eq!(ans.len(), 3);
    assert_eq!(ans[0].0.as_slice(), &[11, 3, 13]);
    assert_eq!(ans[1].0.as_slice(), &[11, 4, 13]);
    assert_eq!(ans[2].0.as_slice(), &[11, 5, 13]);
    assert_eq!(
        ans[0].1.as_slice(),
        &[
            // column major
            1, 0, 0, 0, //
            0, 1, 0, 0, //
            0, 0, 1, 0, //
            0, 0, 0, 1, //
        ]
    );
    assert_eq!(
        ans[1].1.as_slice(),
        &[
            // column major
            1, 0, 0, 0, //
            0, 1, 0, 3, //
            0, 0, 1, 0, //
            0, 0, 0, 1, //
        ]
    );
    assert_eq!(
        ans[2].1.as_slice(),
        &[
            // column major
            1, 0, 0, 0, //
            0, 1, 0, 7, //
            0, 0, 1, 0, //
            0, 0, 0, 1, //
        ]
    );
}

#[macro_export]
macro_rules! split {
    ($src:expr; [$axis:expr]: $($n:expr),+) => {
        {
            let mut vec = $src.split($axis, &[$($n as _),+]);
            ($((vec.pop_front().unwrap(),$n).0,)+)
        }
    };
}

#[test]
fn test_macro() {
    use digit_layout::types::U8;
    let (_a, _b, _c) = split!(Tensor::new(U8, &[10], ()); [0]: 2, 3, 4);
}
