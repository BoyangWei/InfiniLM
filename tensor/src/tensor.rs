﻿use crate::{expand_indices, idim, idx_strides, pattern::Pattern, udim, Compatibility, Shape};
use digit_layout::DigitLayout;
use nalgebra::DVector;
use rayon::iter::*;
use std::{
    mem::{align_of, size_of},
    ops::{Deref, DerefMut},
    panic,
};

#[derive(Clone, Debug)]
pub struct Tensor<Physical> {
    pub(crate) layout: DigitLayout,
    pub(crate) shape: Shape,
    pub(crate) pattern: Pattern,
    pub(crate) physical: Physical,
}

impl<Physical> Tensor<Physical> {
    #[inline]
    pub fn new(layout: DigitLayout, shape: &[udim], physical: Physical) -> Self {
        Self {
            layout,
            pattern: Pattern::from_shape(shape, 0),
            shape: Shape::from_slice(shape),
            physical,
        }
    }

    #[inline]
    pub fn alloc(
        data_type: DigitLayout,
        shape: &[udim],
        f: impl FnOnce(usize) -> Physical,
    ) -> Self {
        Self {
            layout: data_type,
            pattern: Pattern::from_shape(shape, 0),
            shape: Shape::from_slice(shape),
            physical: f(shape.iter().product::<udim>() as usize * data_type.nbytes()),
        }
    }

    /// # Safety
    ///
    /// The caller must ensure that the parts are valid.
    #[inline]
    pub unsafe fn from_raw_parts(
        data_type: DigitLayout,
        shape: &[udim],
        pattern: &[idim],
        physical: Physical,
    ) -> Self {
        Self {
            layout: data_type,
            shape: shape.iter().copied().collect(),
            pattern: Pattern(DVector::from_vec(pattern.to_vec())),
            physical,
        }
    }

    #[inline]
    pub const fn data_layout(&self) -> DigitLayout {
        self.layout
    }

    #[inline]
    pub fn shape(&self) -> &[udim] {
        &self.shape
    }

    #[inline]
    pub fn pattern(&self) -> &[idim] {
        self.pattern.0.as_slice()
    }

    #[inline]
    pub fn strides(&self) -> &[idim] {
        self.pattern.strides()
    }

    #[inline]
    pub fn bytes_offset(&self) -> isize {
        self.pattern.offset() as isize * self.layout.nbytes() as isize
    }

    #[inline]
    pub const fn physical(&self) -> &Physical {
        &self.physical
    }

    #[inline]
    pub fn physical_mut(&mut self) -> &mut Physical {
        &mut self.physical
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.shape.iter().map(|&d| d as usize).product()
    }

    #[inline]
    pub fn bytes_size(&self) -> usize {
        self.size() * self.layout.nbytes()
    }

    #[inline]
    pub fn is_contiguous(&self) -> bool {
        self.contiguous_len() == self.shape.len()
    }

    /// 连续维度的数量。
    pub fn contiguous_len(&self) -> usize {
        self.pattern
            .strides()
            .iter()
            .enumerate()
            .rev()
            .scan(1 as idim, |mul, (i, &s)| {
                if s == *mul || s == 0 {
                    *mul *= self.shape[i] as idim;
                    Some(())
                } else {
                    None
                }
            })
            .count()
    }

    #[inline]
    pub fn as_ref(&self) -> Tensor<&Physical> {
        Tensor {
            layout: self.layout,
            shape: self.shape.clone(),
            pattern: self.pattern.clone(),
            physical: &self.physical,
        }
    }

    #[inline]
    pub fn as_mut(&mut self) -> Tensor<&mut Physical> {
        Tensor {
            layout: self.layout,
            shape: self.shape.clone(),
            pattern: self.pattern.clone(),
            physical: &mut self.physical,
        }
    }

    #[inline]
    pub fn take_physical(self) -> Physical {
        self.physical
    }

    #[inline]
    pub fn map_physical<U>(self, f: impl FnOnce(Physical) -> U) -> Tensor<U> {
        Tensor {
            layout: self.layout,
            shape: self.shape,
            pattern: self.pattern,
            physical: f(self.physical),
        }
    }
}

impl<B: Sized, P: Deref<Target = [B]>> Tensor<P> {
    pub fn base(&self) -> *const B {
        const { assert!(size_of::<B>() == 1) }
        const { assert!(align_of::<B>() == 1) }

        let off = self.bytes_offset();
        unsafe { self.physical.as_ptr().cast::<u8>().offset(off).cast() }
    }
}

impl<B: Sized, P: DerefMut<Target = [B]>> Tensor<P> {
    pub fn base_mut(&mut self) -> *mut B {
        const { assert!(size_of::<B>() == 1) }
        const { assert!(align_of::<B>() == 1) }

        let off = self.bytes_offset();
        unsafe {
            self.physical_mut()
                .as_mut_ptr()
                .cast::<u8>()
                .offset(off)
                .cast()
        }
    }
}

impl<Physical: Deref<Target = [u8]>> Tensor<Physical> {
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        debug_assert!(self.is_contiguous());
        let off = self.bytes_offset();
        let len = self.bytes_size();
        &self.physical[off as usize..][..len]
    }

    /// # Safety
    ///
    /// The caller must ensure that the `dst` can be a valid tensor physical.
    pub unsafe fn reform_to_raw(&self, dst: &mut [u8]) {
        let src = &self.physical[self.bytes_offset() as usize..];
        // 计算结尾连续维度数量
        let contiguous = self.contiguous_len();
        if contiguous == self.shape.len() {
            // 所有维度都连续，直接拷贝所有数据
            dst.copy_from_slice(&src[..dst.len()]);
        } else {
            let dt = self.layout.nbytes();
            // 一部分维度连续，迭代不连续的部分
            let (iter, contiguous) = self.shape.split_at(self.shape.len() - contiguous);
            let (n, idx_strides) = idx_strides(iter);
            let len = contiguous.iter().product::<udim>() as usize * dt;
            let pattern = self.pattern.0.view_range(..iter.len(), ..);
            let ptr = dst.as_mut_ptr() as usize;
            (0..n).into_par_iter().for_each(|i| {
                let j = pattern.dot(&expand_indices(i, &idx_strides, &[]));
                unsafe { std::slice::from_raw_parts_mut((ptr + i as usize * len) as *mut u8, len) }
                    .copy_from_slice(&src[j as usize * dt..][..len]);
            });
        }
    }

    pub fn reform_to<U>(&self, dst: &mut Tensor<U>)
    where
        U: DerefMut<Target = [u8]>,
    {
        match Compatibility::between(self, dst) {
            Compatibility::None => panic!("Incompatible tensors"),
            _ => {
                let contiguous = self.contiguous_len().min(dst.contiguous_len());
                let dt = self.layout.nbytes();
                // 一部分维度连续，迭代不连续的部分
                let (iter, contiguous) = self.shape.split_at(self.shape.len() - contiguous);
                let (n, idx_strides) = idx_strides(iter);
                let src_pattern = self.pattern.0.view_range(..iter.len(), ..);
                let dst_pattern = dst.pattern.0.view_range(..iter.len(), ..);
                let src = self.base() as usize;
                let dst = dst.base() as usize;
                let count = contiguous.iter().product::<udim>() as usize * dt;
                (0..n).into_par_iter().for_each(|i| {
                    let indices = expand_indices(i, &idx_strides, &[]);
                    let src = (src + src_pattern.dot(&indices) as usize * dt) as *const u8;
                    let dst = (dst + dst_pattern.dot(&indices) as usize * dt) as *mut u8;
                    unsafe { std::ptr::copy_nonoverlapping(src, dst, count) };
                });
            }
        }
    }
}

impl<Physical: DerefMut<Target = [u8]>> Tensor<Physical> {
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        debug_assert!(self.is_contiguous());
        let off = self.bytes_offset();
        let len = self.bytes_size();
        &mut self.physical[off as usize..][..len]
    }
}

#[test]
fn test() {
    use digit_layout::types::F32;

    let t = Tensor::new(F32, &[2, 3, 4, 5], ());
    assert_eq!(t.shape(), &[2, 3, 4, 5]);
    assert_eq!(t.pattern.0.as_slice(), &[60, 20, 5, 1, 0]);
    assert_eq!(t.contiguous_len(), 4);
    assert_eq!(t.is_contiguous(), true);

    let t = t.reshape(&[2, 3, 20]);
    assert_eq!(t.shape(), &[2, 3, 20]);
    assert_eq!(t.pattern.0.as_slice(), &[60, 20, 1, 0]);
    assert_eq!(t.contiguous_len(), 3);
    assert_eq!(t.is_contiguous(), true);

    let t = t.transpose(&[1, 0, 2]);
    assert_eq!(t.shape(), &[3, 2, 20]);
    assert_eq!(t.pattern.0.as_slice(), &[20, 60, 1, 0]);
    assert_eq!(t.contiguous_len(), 1);
    assert_eq!(t.is_contiguous(), false);

    let t = t.reshape(&[3, 1, 1, 2, 5, 1, 4, 1, 1, 1]);
    assert_eq!(t.shape(), &[3, 1, 1, 2, 5, 1, 4, 1, 1, 1]);
    assert_eq!(t.pattern.0.as_slice(), &[20, 0, 0, 60, 4, 0, 1, 0, 0, 0, 0]);
    assert_eq!(t.contiguous_len(), 6);
    assert_eq!(t.is_contiguous(), false);

    let t = t.reshape(&[3, 2, 1, 5, 2, 2]);
    assert_eq!(t.shape(), &[3, 2, 1, 5, 2, 2]);
    assert_eq!(t.pattern.0.as_slice(), &[20, 60, 0, 4, 2, 1, 0]);
    assert_eq!(t.contiguous_len(), 4);
    assert_eq!(t.is_contiguous(), false);
}
