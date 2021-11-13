//! `VMem` provides a virtual memory data structure where physical memory is only allocated
//! as it is written to.
//!
//! # Example :
//! ```
//! use vmem::VMem;
//! // Create a new virtual memory that spans the addresses 0x00 to 0xff,
//! // where each word is 4 bytes wide.
//! let mut vmem = VMem::<4>::new(0x100);
//!
//! // Write a word to the specified address.
//! vmem.write_word([0x01, 0x02, 0x04, 0x08], 0x03).unwrap();
//!
//! // Read word from the specified address.
//! let word = vmem.read_word(0x03);
//!
//! ```

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::io::{self, ErrorKind, Write};
use std::iter::IntoIterator;

/// Virtual memory data structure.
#[derive(Debug, PartialEq)]
pub struct VMem<const W: usize> {
    memory: BTreeMap<usize, [u8; W]>,
    len: usize,
}

impl<const W: usize> VMem<W> {
    /// Create a new [`VMem`] with a given length (the number of words).
    pub fn new(length: usize) -> Self {
        Self {
            memory: BTreeMap::new(),
            len: length,
        }
    }

    /// The length of the `VMem`;
    pub fn len(&self) -> usize {
        self.len
    }

    /// The width of the `VMem`
    pub fn width(&self) -> usize {
        W
    }

    /// Read word at a the specified address.
    pub fn read_word(&self, addr: usize) -> Option<[u8; W]> {
        if addr < self.len {
            match self.memory.get(&addr) {
                Some(word) => Some(*word),
                None => Some([0x00; W]),
            }
        } else {
            None
        }
    }

    /// Write word to the specified address.
    pub fn write_word(&mut self, word: [u8; W], addr: usize) -> Result<(), ErrorKind> {
        if addr < self.len {
            self.memory.insert(addr, word);
            Ok(())
        } else {
            Err(ErrorKind::AddrNotAvailable)
        }
    }

    /// Fill buffer with the contents from the `VMem` starting from the specified address.
    ///
    /// # Example
    /// ```
    /// use vmem::VMem;
    ///
    /// let mut vmem = VMem::<4>::new(0x0f);
    /// vmem.write_word([0x01, 0x02, 0x04, 0x08], 0x0d);
    ///
    /// let mut buf = [0x00; 8];
    /// vmem.read_at(&mut buf, 0x0d).unwrap();
    ///
    /// let expected_buf = [
    ///     0x01, 0x02, 0x04, 0x08,
    ///     0x00, 0x00, 0x00, 0x00,
    /// ];
    ///
    /// assert_eq!(buf, expected_buf);
    /// ```
    pub fn read_at(&self, buf: &mut [u8], mut addr: usize) -> io::Result<usize> {
        let mut chunk_iterator = buf.chunks_exact_mut(W);
        for mut chunk in &mut chunk_iterator {
            if addr < self.len {
                let _ = chunk.write(&self.read_word(addr).unwrap());
            }
            addr += 1;
        }
        let mut bytes_read = addr * W;
        let mut remainder = chunk_iterator.into_remainder();
        if let (false, true) = (remainder.is_empty(), addr < self.len) {
            let _ = remainder.write(&self.read_word(addr).unwrap());
        }
        bytes_read += remainder.len();
        Ok(bytes_read)
    }

    /// Write the content from the buffer to the `VMem` starting from the specified address.
    pub fn write_at(&mut self, buf: &[u8], mut addr: usize) {
        let mut chunk_iterator = buf.chunks_exact(W);
        for chunk in &mut chunk_iterator {
            if addr < self.len {
                self.memory.insert(addr, chunk.try_into().unwrap());
            }
            addr += 1;
        }
        let remainder = chunk_iterator.remainder();
        if let (false, true) = (remainder.is_empty(), addr < self.len) {
            match self.memory.entry(addr) {
                Entry::Vacant(block) => {
                    let mut temp = [0x00; W];
                    (&mut temp[..]).write(remainder).unwrap();
                    block.insert(temp.try_into().unwrap());
                }
                Entry::Occupied(block) => {
                    (&mut block.into_mut()[..]).write(remainder).unwrap();
                }
            }
        }
    }

    /// Diff two [`VMem`]s, returning a BTreeMap with (address, word) pairs.
    pub fn diff(old: &Self, new: &Self) -> BTreeMap<usize, [u8; W]> {
        let mut diff = BTreeMap::<usize, [u8; W]>::new();
        let zipped = old.iter().zip(new.iter());
        for (addr, (old_word, new_word)) in zipped.enumerate() {
            if old_word != new_word {
                diff.insert(addr, *new_word);
            }
        }
        diff
    }

    /// Apply a patch with new data to the [`VMem`]
    pub fn patch(&mut self, patch: BTreeMap<usize, [u8; W]>) -> Result<(), ErrorKind> {
        for (addr, word) in patch.into_iter() {
            self.write_word(word, addr)?;
        }
        Ok(())
    }

    /// Iterate over references to the words inside the 'VMem'
    pub fn iter<'a>(&'a self) -> Iter<'a, W> {
        self.into_iter()
    }

    /// Iterate over mutable references to the words inside the 'VMem'
    ///
    ///  **Warning**: This will allocate words that hadn't been written to.
    pub fn iter_mut<'a>(&'a mut self) -> IterMut<'a, W> {
        self.into_iter()
    }

    ///
    pub fn iter_content<'a>(&'a self) -> std::collections::btree_map::Iter<'a, usize, [u8; W]> {
        self.memory.iter()
    }

    /// Iterate over adjacent chunks of a specified maximum size.
    pub fn chunks_adjacent_content<'a>(
        &'a self,
        chunk_size: usize,
    ) -> ChunksAdjacentContent<'a, W> {
        ChunksAdjacentContent {
            iter: self.memory.iter().peekable(),
            chunk_size,
        }
    }
}

impl<const W: usize> IntoIterator for VMem<W> {
    type Item = [u8; W];
    type IntoIter = IntoIter<W>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            memory: self.memory,
            length: self.len,
            index: 0,
        }
    }
}

impl<'a, const W: usize> IntoIterator for &'a VMem<W> {
    type Item = &'a [u8; W];
    type IntoIter = Iter<'a, W>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            memory: &self.memory,
            length: self.len,
            index: 0,
        }
    }
}

impl<'a, const W: usize> IntoIterator for &'a mut VMem<W> {
    type Item = &'a mut [u8; W];
    type IntoIter = IterMut<'a, W>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            memory: &mut self.memory,
            length: self.len,
            index: 0,
        }
    }
}

impl<const W: usize> From<&[u8]> for VMem<W> {
    fn from(bytes: &[u8]) -> Self {
        let len = bytes.len() / W + if bytes.len() % W != 0 { 1 } else { 0 };
        let mut vmem = VMem::<W>::new(len);
        for (addr, word) in bytes.chunks_exact(W).enumerate() {
            if word != [0x00; W] {
                vmem.write_word(word.try_into().unwrap(), addr).unwrap();
            }
        }
        vmem
    }
}

/// Iterator that takes ownership of [`VMem`].
pub struct IntoIter<const W: usize> {
    memory: BTreeMap<usize, [u8; W]>,
    length: usize,
    index: usize,
}

impl<const W: usize> Iterator for IntoIter<W> {
    type Item = [u8; W];

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.length {
            let word = match self.memory.get(&self.index) {
                Some(word) => Some(*word),
                None => Some([0x00; W]),
            };
            self.index += 1;
            word
        } else {
            None
        }
    }
}

/// Iterator over a reference of [`VMem`].
pub struct Iter<'a, const W: usize> {
    memory: &'a BTreeMap<usize, [u8; W]>,
    length: usize,
    index: usize,
}

impl<'a, const W: usize> Iterator for Iter<'a, W> {
    type Item = &'a [u8; W];

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.length {
            let word = match self.memory.get(&self.index) {
                Some(word) => Some(word),
                None => Some(&[0x00; W]),
            };
            self.index += 1;
            word
        } else {
            None
        }
    }
}

/// Iterator over a mutable reference of [`VMem`].
pub struct IterMut<'a, const W: usize> {
    memory: &'a mut BTreeMap<usize, [u8; W]>,
    length: usize,
    index: usize,
}

impl<'a, const W: usize> Iterator for IterMut<'a, W> {
    type Item = &'a mut [u8; W];

    fn next(&mut self) -> Option<Self::Item> {
        let word = if self.index < self.length {
            let entry = match self.memory.entry(self.index) {
                Entry::Vacant(block) => {
                    let temp = [0x00; W];
                    &mut *block.insert(temp.try_into().unwrap())
                }
                Entry::Occupied(block) => block.into_mut(),
            };
            unsafe { Some(std::mem::transmute(&mut *entry)) }
        } else {
            None
        };
        self.index += 1;
        word
    }
}

pub struct ChunksAdjacentContent<'a, const W: usize> {
    iter: std::iter::Peekable<std::collections::btree_map::Iter<'a, usize, [u8; W]>>,
    chunk_size: usize,
}

impl<'a, const W: usize> Iterator for ChunksAdjacentContent<'a, W> {
    type Item = Vec<(&'a usize, &'a [u8; W])>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(start) = self.iter.next() {
            let mut chunk = Vec::with_capacity(self.chunk_size);
            chunk.push(start);
            for i in 1..self.chunk_size {
                if let Some((addr, _)) = self.iter.peek() {
                    if **addr == start.0 + i {
                        chunk.push(self.iter.next().unwrap());
                    }
                }
            }
            Some(chunk)
        } else {
            None
        }
    }
}

#[test]
fn read_word() {
    let mut vmem = VMem::<4>::new(0x0f);
    let word = [0x0a, 0x0b, 0x0c, 0x0d];
    vmem.write_word(word, 0x03).unwrap();
    let read = vmem.read_word(0x03).unwrap();
    assert_eq!(word, read);
    let read = vmem.read_word(0x0f);
    assert_eq!(read, None);
}

#[test]
fn write_word() {
    let mut vmem = VMem::<4>::new(0x0f);
    let word = [0x0a, 0x0b, 0x0c, 0x0d];
    let result = vmem.write_word(word, 0x03);
    assert_eq!(result, Ok(()));
    let result = vmem.write_word(word, 0x0e);
    assert_eq!(result, Ok(()));
    let result = vmem.write_word(word, 0x0f);
    assert_eq!(result, Err(ErrorKind::AddrNotAvailable));
}

#[test]
fn read_at() {
    let mut vmem = VMem::<4>::new(0x0f);

    #[rustfmt::skip]
    let data = [
        0x01, 0x02, 0x03, 0x04,
        0x01, 0x02, 0x03, 0x04, 
        0x05
    ];

    vmem.write_at(&data, 0x0d);
    let mut buf = [0x00; 8];
    vmem.read_at(&mut buf, 0x0d).unwrap();

    #[rustfmt::skip]
    let expected_buf = [
        0x01, 0x02, 0x03, 0x04,
        0x01, 0x02, 0x03, 0x04
    ];

    assert_eq!(buf, expected_buf);
}

#[test]
fn write_at() {
    let mut vmem = VMem::<4>::new(0x0f);

    #[rustfmt::skip]
    let data = [
        0x01, 0x02, 0x03, 0x04,
        0x01, 0x02, 0x03, 0x04,
        0x05
    ];

    vmem.write_at(&data, 0x0d);
    vmem.write_at(&data, 0x02);
    let mut buf = [0x00u8; 9];
    vmem.read_at(&mut buf, 0x2).unwrap();

    #[rustfmt::skip]
    let expected_buf = [
        0x01, 0x02, 0x03, 0x04, 
        0x01, 0x02, 0x03, 0x04, 
        0x05
    ];

    assert_eq!(buf, expected_buf);
    let mut buf = [0x00u8; 9];
    vmem.read_at(&mut buf, 0x0d).unwrap();

    #[rustfmt::skip]
    let expected_buf = [
        0x01, 0x02, 0x03, 0x04,
        0x01, 0x02, 0x03, 0x04,
        0x00
    ];
    assert_eq!(buf, expected_buf);
}

#[test]
fn into_iter() {
    let mut vmem = VMem::<4>::new(8);

    #[rustfmt::skip]
    let data = [
        0x01, 0x02, 0x03, 0x04, 
        0x01, 0x02, 0x03, 0x04, 
        0x05
    ];

    vmem.write_at(&data, 0x02);
    vmem.write_at(&data, 0x06);

    let mut iter = vmem.into_iter();
    assert_eq!(iter.next(), Some([0x00, 0x00, 0x00, 0x00]));
    assert_eq!(iter.next(), Some([0x00, 0x00, 0x00, 0x00]));
    assert_eq!(iter.next(), Some([0x01, 0x02, 0x03, 0x04]));
    assert_eq!(iter.next(), Some([0x01, 0x02, 0x03, 0x04]));

    assert_eq!(iter.next(), Some([0x05, 0x00, 0x00, 0x00]));
    assert_eq!(iter.next(), Some([0x00, 0x00, 0x00, 0x00]));
    assert_eq!(iter.next(), Some([0x01, 0x02, 0x03, 0x04]));
    assert_eq!(iter.next(), Some([0x01, 0x02, 0x03, 0x04]));

    assert_eq!(iter.next(), None);
}

#[test]
fn iter() {
    let mut vmem = VMem::<4>::new(8);

    #[rustfmt::skip]
    let data = [
        0x01, 0x02, 0x03, 0x04,
        0x01, 0x02, 0x03, 0x04,
        0x05
    ];

    vmem.write_at(&data, 0x02);
    vmem.write_at(&data, 0x06);

    let mut iter = vmem.iter();
    assert_eq!(iter.next(), Some(&[0x00, 0x00, 0x00, 0x00]));
    assert_eq!(iter.next(), Some(&[0x00, 0x00, 0x00, 0x00]));
    assert_eq!(iter.next(), Some(&[0x01, 0x02, 0x03, 0x04]));
    assert_eq!(iter.next(), Some(&[0x01, 0x02, 0x03, 0x04]));

    assert_eq!(iter.next(), Some(&[0x05, 0x00, 0x00, 0x00]));
    assert_eq!(iter.next(), Some(&[0x00, 0x00, 0x00, 0x00]));
    assert_eq!(iter.next(), Some(&[0x01, 0x02, 0x03, 0x04]));
    assert_eq!(iter.next(), Some(&[0x01, 0x02, 0x03, 0x04]));

    assert_eq!(iter.next(), None);
}

#[test]
fn iter_mut() {
    let mut vmem = VMem::<4>::new(0x0f);
    for word in vmem.iter_mut() {
        *word = [0x01, 0x02, 0x03, 0x04];
    }
    for word in &vmem {
        assert_eq!(*word, [0x01, 0x02, 0x03, 0x04]);
    }
}

#[test]
fn iter_chunk_adjecent_content() {
    let mut vmem = VMem::<4>::new(0x0f);
    vmem.write_word([0x01, 0x02, 0x03, 0x04], 2).unwrap();
    vmem.write_word([0x02, 0x04, 0x08, 0x16], 3).unwrap();
    vmem.write_word([0x01, 0x02, 0x03, 0x04], 8).unwrap();
    vmem.write_word([0x02, 0x04, 0x08, 0x16], 9).unwrap();
    vmem.write_word([0x01, 0x02, 0x03, 0x04], 10).unwrap();
    vmem.write_word([0x02, 0x04, 0x08, 0x16], 11).unwrap();
    for chunk in vmem.chunks_adjacent_content(3) {
        dbg!(chunk
            .iter()
            .map(|(_, &word)| word)
            .collect::<Vec<[u8; 4]>>()
            .concat());
    }
}

#[test]
fn diff() {
    let mut vmem_old = VMem::<4>::new(8);

    #[rustfmt::skip]
    let data = [
        0x01, 0x02, 0x03, 0x04, 
        0x01, 0x02, 0x03, 0x04, 
        0x05
    ];

    vmem_old.write_at(&data, 0x02);
    let mut vmem_new = VMem::<4>::new(8);
    vmem_new.write_at(&data, 0x02);
    vmem_new.write_at(&data, 0x06);

    let patch = VMem::diff(&vmem_old, &vmem_new);
    let mut expected_patch = BTreeMap::new();
    expected_patch.insert(0x06, [0x01, 0x02, 0x03, 0x04]);
    expected_patch.insert(0x07, [0x01, 0x02, 0x03, 0x04]);
    assert_eq!(patch, expected_patch);
}
