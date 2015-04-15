///! A sparse matrix in the Compressed Sparse Row/Column format
///
/// In the CSR format, a matrix is a structure containing three vectors:
/// indptr, indices, and data
/// These vectors satisfy the relation
/// for i in [0, nrows],
/// A(i, indices[indptr[i]..indptr[i+1]]) = data[indptr[i]..indptr[i+1]]
/// In the CSC format, the relation is
/// A(indices[indptr[i]..indptr[i+1]], i) = data[indptr[i]..indptr[i+1]]

use std::iter::{Enumerate};
use std::slice::{Windows};
use std::ops::{Deref};

use sparse::permutation::{Permutation};
use sparse::vec::{CsVec};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CompressedStorage {
    CSR,
    CSC
}

use self::CompressedStorage::*;

/// Iterator on the matrix' outer dimension
/// Implemented over an iterator on the indptr array
pub struct OuterIterator<'iter, 'perm: 'iter, N: 'iter> {
    indptr_iter: Enumerate<Windows<'iter, usize>>,
    indices: &'iter [usize],
    data: &'iter [N],
    perm: Permutation<&'perm[usize]>,
}

/// Outer iteration on a compressed matrix yields
/// a tuple consisting of the outer index and of a sparse vector
/// containing the associated inner dimension
impl <'res, 'iter: 'res, 'perm: 'iter, N: 'iter + Clone>
Iterator
for OuterIterator<'iter, 'perm, N> {
    type Item = (usize, CsVec<'res, N, &'res[usize], &'res[N]>);
    #[inline]
    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        match self.indptr_iter.next() {
            None => None,
            Some((outer_ind, window)) => {
                let inner_start = window[0];
                let inner_end = window[1];
                let outer_ind_perm = self.perm.at(outer_ind);
                let indices = &self.indices[inner_start..inner_end];
                let data = &self.data[inner_start..inner_end];
                let vec = CsVec::new_borrowed(
                    indices, data, self.perm.borrowed());
                Some((outer_ind_perm, vec))
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indptr_iter.size_hint()
    }
}

/// Reverse outer iteration on a compressed matrix yields
/// a tuple consisting of the outer index and of a sparse vector
/// containing the associated inner dimension
///
/// Only the outer dimension iteration is reverted. If you wish to also
/// revert the inner dimension, you should call rev() again when iterating
/// the vector.
impl <'res, 'iter: 'res, 'perm: 'iter, N: 'iter + Clone>
DoubleEndedIterator
for OuterIterator<'iter, 'perm, N> {
    #[inline]
    fn next_back(&mut self) -> Option<<Self as Iterator>::Item> {
        match self.indptr_iter.next_back() {
            None => None,
            Some((outer_ind, window)) => {
                let inner_start = window[0];
                let inner_end = window[1];
                let outer_ind_perm = self.perm.at(outer_ind);
                let indices = &self.indices[inner_start..inner_end];
                let data = &self.data[inner_start..inner_end];
                let vec = CsVec::new_borrowed(
                    indices, data, self.perm.borrowed());
                Some((outer_ind_perm, vec))
            }
        }
    }
}

impl <'iter, 'perm: 'iter, N: 'iter + Clone>
ExactSizeIterator
for OuterIterator<'iter, 'perm, N> {
    fn len(&self) -> usize {
        self.indptr_iter.len()
    }
}

pub struct CsMat<N, IndStorage, DataStorage>
where IndStorage: Deref<Target=[usize]>, DataStorage: Deref<Target=[N]> {
    storage: CompressedStorage,
    nrows : usize,
    ncols : usize,
    nnz : usize,
    indptr : IndStorage,
    indices : IndStorage,
    data : DataStorage,
    perm_identity: Permutation<&'static [usize]>
}

impl<'a, N:'a + Clone> CsMat<N, &'a[usize], &'a[N]> {
    /// Create a borrowed CsMat matrix from sliced data,
    /// checking their validity
    pub fn from_slices(
        storage: CompressedStorage, nrows : usize, ncols: usize,
        indptr : &'a[usize], indices : &'a[usize], data : &'a[N]
        )
    -> Option<CsMat<N, &'a[usize], &'a[N]>> {
        let m = CsMat {
            storage: storage,
            nrows : nrows,
            ncols: ncols,
            nnz : data.len(),
            indptr : indptr,
            indices : indices,
            data : data,
            perm_identity : Permutation::identity(),
        };
        match m.check_compressed_structure() {
            None => None,
            _ => Some(m)
        }
    }
}

impl<N: Clone> CsMat<N, Vec<usize>, Vec<N>> {
    /// Create an owned CsMat matrix from moved data,
    /// checking their validity
    pub fn from_vecs(
        storage: CompressedStorage, nrows : usize, ncols: usize,
        indptr : Vec<usize>, indices : Vec<usize>, data : Vec<N>
        )
    -> Option<CsMat<N, Vec<usize>, Vec<N>>> {
        let m = CsMat {
            storage: storage,
            nrows : nrows,
            ncols: ncols,
            nnz : data.len(),
            indptr : indptr,
            indices : indices,
            data : data,
            perm_identity : Permutation::identity(),
        };
        match m.check_compressed_structure() {
            None => None,
            _ => Some(m)
        }
    }
}

impl<N: Clone, IndStorage: Deref<Target=[usize]>, DataStorage: Deref<Target=[N]>>
CsMat<N, IndStorage, DataStorage> {

    /// Return an outer iterator for the matrix
    pub fn outer_iterator<'a>(&'a self) -> OuterIterator<'a, 'a, N> {
        self.outer_iterator_papt(&self.perm_identity)
    }

    /// Return an outer iterator over P*A*P^T
    pub fn outer_iterator_papt<'a, 'perm: 'a>(
        &'a self, perm: &'perm Permutation<&'perm [usize]>)
    -> OuterIterator<'a, 'perm, N> {
        let oriented_perm = match self.storage {
            CSR => perm.borrowed(),
            CSC => Permutation::inv(perm)
        };
        OuterIterator {
            indptr_iter: self.indptr.windows(2).enumerate(),
            indices: &self.indices[..],
            data: &self.data[..],
            perm: oriented_perm
        }
    }

    pub fn storage_type(&self) -> CompressedStorage {
        self.storage
    }

    pub fn rows(&self) -> usize {
        self.nrows
    }

    pub fn cols(&self) -> usize {
        self.ncols
    }

    pub fn at(&self, &(i,j) : &(usize, usize)) -> Option<N> {
        assert!(i < self.nrows);
        assert!(j < self.ncols);

        match self.storage {
            CSR => self.at_outer_inner(&(i,j)),
            CSC => self.at_outer_inner(&(j,i))
        }
    }

    pub fn at_outer_inner(&self, &(outer_ind, inner_ind): &(usize, usize))
    -> Option<N> {
        let begin = self.indptr[outer_ind];
        let end = self.indptr[outer_ind+1];
        if begin >= end {
            return None;
        }
        let indices = &self.indices[begin..end];
        let data = &self.data[begin..end];

        let position = match indices.binary_search(&inner_ind) {
            Ok(ind) => ind,
            _ => return None
        };

        Some(data[position].clone())
    }

    /// Check the structure of CsMat components
    fn check_compressed_structure(&self) -> Option<usize> {
        let inner = match self.storage {
            CompressedStorage::CSR => self.ncols,
            CompressedStorage::CSC => self.nrows
        };
        let outer = match self.storage {
            CompressedStorage::CSR => self.nrows,
            CompressedStorage::CSC => self.ncols
        };
        if self.indptr.len() != outer + 1 {
            println!("CsMat indptr length incorrect");
            return None;
        }
        if self.indices.len() != self.data.len() {
            println!("CsMat indices/data length incorrect");
            return None;
        }
        let nnz = self.indices.len();
        if nnz != self.nnz {
            println!("CsMat nnz count incorrect");
            return None;
        }
        if self.indptr.iter().max().unwrap() > &nnz {
            println!("CsMat indptr values incoherent with nnz");
            return None;
        }
        if self.indices.iter().max().unwrap() >= &inner {
            println!("CsMat indices values incoherent with ncols");
            return None;
        }

        if ! self.indptr.deref().windows(2).all(|x| x[0] <= x[1]) {
            println!("CsMat indptr not sorted");
            return None;
        }

        // check that the indices are sorted for each row
        if ! self.outer_iterator().all(
            | (_, vec) | { vec.check_structure() })
        {
            println!("CsMat indices not sorted for each outer ind");
            return None;
        }

        Some(nnz)
    }
}


#[cfg(test)]
mod test {
    use super::{CsMat};
    use super::CompressedStorage::{CSC, CSR};

    #[test]
    fn test_new_csr_success() {
        let indptr_ok : &[usize] = &[0, 1, 2, 3];
        let indices_ok : &[usize] = &[0, 1, 2];
        let data_ok : &[f64] = &[1., 1., 1.];
        match CsMat::from_slices(CSR, 3, 3, indptr_ok, indices_ok, data_ok) {
            Some(_) => assert!(true),
            None => assert!(false)
        }
    }

    #[test]
    fn test_new_csr_fails() {
        let indptr_ok : &[usize] = &[0, 1, 2, 3];
        let indices_ok : &[usize] = &[0, 1, 2];
        let data_ok : &[f64] = &[1., 1., 1.];
        let indptr_fail1 : &[usize] = &[0, 1, 2];
        let indptr_fail2 : &[usize] = &[0, 1, 2, 4];
        let indptr_fail3 : &[usize] = &[0, 2, 1, 3];
        let indices_fail1 : &[usize] = &[0, 1];
        let indices_fail2 : &[usize] = &[0, 1, 4];
        let data_fail1 : &[f64] = &[1., 1., 1., 1.];
        let data_fail2 : &[f64] = &[1., 1.,];
        match CsMat::from_slices(CSR, 3, 3, indptr_fail1, indices_ok, data_ok) {
            Some(_) => assert!(false),
            None => assert!(true)
        }
        match CsMat::from_slices(CSR, 3, 3, indptr_fail2, indices_ok, data_ok) {
            Some(_) => assert!(false),
            None => assert!(true)
        }
        match CsMat::from_slices(CSR, 3, 3, indptr_fail3, indices_ok, data_ok) {
            Some(_) => assert!(false),
            None => assert!(true)
        }
        match CsMat::from_slices(CSR, 3, 3, indptr_ok, indices_fail1, data_ok) {
            Some(_) => assert!(false),
            None => assert!(true)
        }
        match CsMat::from_slices(CSR, 3, 3, indptr_ok, indices_fail2, data_ok) {
            Some(_) => assert!(false),
            None => assert!(true)
        }
        match CsMat::from_slices(CSR, 3, 3, indptr_ok, indices_ok, data_fail1) {
            Some(_) => assert!(false),
            None => assert!(true)
        }
        match CsMat::from_slices(CSR, 3, 3, indptr_ok, indices_ok, data_fail2) {
            Some(_) => assert!(false),
            None => assert!(true)
        }
    }

    #[test]
    fn test_new_csr_fail_indices_ordering() {
        let indptr: &[usize] = &[0, 2, 4, 5, 6, 7];
        // good indices would be [2, 3, 3, 4, 2, 1, 3];
        let indices: &[usize] = &[3, 2, 3, 4, 2, 1, 3];
        let data: &[f64] = &[
            0.35310881, 0.42380633, 0.28035896, 0.58082095,
            0.53350123, 0.88132896, 0.72527863];
        match CsMat::from_slices(CSR, 5, 5, indptr, indices, data) {
            Some(_) => assert!(false),
            None => assert!(true)
        }
    }

    #[test]
    fn test_new_csr_csc_success() {
        let indptr_ok : &[usize] = &[0, 2, 5, 6];
        let indices_ok : &[usize] = &[2, 3, 1, 2, 3, 3];
        let data_ok : &[f64] = &[
            0.05734571, 0.15543348, 0.75628258,
            0.83054515, 0.71851547, 0.46202352];
        match CsMat::from_slices(CSR, 3, 4, indptr_ok, indices_ok, data_ok) {
            Some(_) => assert!(true),
            None => assert!(false)
        }
        match CsMat::from_slices(CSC, 4, 3, indptr_ok, indices_ok, data_ok) {
            Some(_) => assert!(true),
            None => assert!(false)
        }
    }

    #[test]
    fn test_new_csr_csc_fails() {
        let indptr_ok : &[usize] = &[0, 2, 5, 6];
        let indices_ok : &[usize] = &[2, 3, 1, 2, 3, 3];
        let data_ok : &[f64] = &[
            0.05734571, 0.15543348, 0.75628258,
            0.83054515, 0.71851547, 0.46202352];
        match CsMat::from_slices(CSR, 4, 3, indptr_ok, indices_ok, data_ok) {
            Some(_) => assert!(false),
            None => assert!(true)
        }
        match CsMat::from_slices(CSC, 3, 4, indptr_ok, indices_ok, data_ok) {
            Some(_) => assert!(false),
            None => assert!(true)
        }
    }


    #[test]
    fn test_new_csr_vec_borrowed() {
        let indptr_ok = vec![0, 1, 2, 3];
        let indices_ok = vec![0, 1, 2];
        let data_ok : Vec<f64> = vec![1., 1., 1.];
        match CsMat::from_slices(CSR, 3, 3, &indptr_ok, &indices_ok, &data_ok) {
            Some(_) => assert!(true),
            None => assert!(false)
        }
    }

    #[test]
    fn test_new_csr_vec_owned() {
        let indptr_ok = vec![0, 1, 2, 3];
        let indices_ok = vec![0, 1, 2];
        let data_ok : Vec<f64> = vec![1., 1., 1.];
        match CsMat::from_vecs(CSR, 3, 3, indptr_ok, indices_ok, data_ok) {
            Some(_) => assert!(true),
            None => assert!(false)
        }
    }

    #[test]
    fn new_csr_with_empty_row() {
        let indptr: &[usize] = &[0, 3, 3, 5, 6, 7];
        let indices: &[usize] = &[1, 2, 3, 2, 3, 4, 4];
        let data: &[f64] = &[
            0.75672424, 0.1649078, 0.30140296, 0.10358244,
            0.6283315, 0.39244208, 0.57202407];
        match CsMat::from_slices(CSR, 5, 5, indptr, indices, data) {
            Some(_) => assert!(true),
            None => assert!(false)
        }
    }
}