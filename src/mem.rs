use std::io::{Cursor, Read, Write};
use std::ptr;

use byteorder::{ByteOrder, LittleEndian};

use vec_map::VecMap;

const DEFAULT_PAGE_SIZE: usize = 4096;

pub struct Memory {
    page_size: usize,
    pub pages: VecMap<Box<[u8]>>
}

impl Default for Memory {
    /// Creates an empty `Memory` with default page size.
    fn default() -> Self {
        Self::new()
    }
}

impl Memory {
    /// Constructs a new, empty `Memory` with the default page size of 4096 bytes.
    ///
    /// No pages will be allocated until bytes are written to them and reads
    /// will return null bytes.
    pub fn new() -> Self {
        Self {
            page_size: DEFAULT_PAGE_SIZE,
            pages: VecMap::new()
        }
    }

    /// Constructs a new, empty `Memory` with the specified page size in bytes.
    ///
    /// # Panics
    ///
    /// Panics if `page_size` is not a factor of 2^32.
    pub fn with_page_size(page_size: usize) -> Self {
        assert_eq!((1 << 32) % page_size as u64, 0, "`page_size` is not a factor of 2^32");

        Self {
            page_size: page_size,
            pages: VecMap::new()
        }
    }

    /// Returns the number of pages the memory consists of.
    pub fn page_count(&self) -> usize {
        ((u32::max_value() as u64 + 1) / self.page_size as u64) as usize
    }

    /// Returns the size of pages in the memory.
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// Returns a reference to the page at position `index`, or `None` if it is
    /// not already allocated.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than or equal to [`page_count`].
    ///
    /// [`page_count`]: #method.page_count
    #[inline]
    pub fn page(&self, index: usize) -> Option<&[u8]> {
        assert!(index <= self.page_count(), "`index` out of bounds");
        self.pages.get(index).map(|p| p.as_ref())
    }

    /// Returns a mutable reference to the page at position `index`.
    ///
    /// Allocates the page if it wasn't previously.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than or equal to [`page_count`].
    ///
    /// [`page_count`]: #method.page_count
    #[inline]
    pub fn page_mut(&mut self, index: usize) -> &mut [u8] {
        assert!(index <= self.page_count(), "`index` out of bounds");
        
        // Get `page_size` first to avoid current limitations in borrowck
        let page_size = self.page_size();
        self.pages.entry(index).or_insert(vec![0; page_size].into()).as_mut()
    }

    /// Reads bytes into the specified buffer `buf` starting at byte address `addr`.
    pub fn read(&self, addr: u32, buf: &mut [u8]) {
        let end_addr = addr as u64 + (buf.len() as u64).saturating_sub(1);

        if end_addr == addr as u64 {
            return;
        }

        // Reads may span multiple pages, so we get a range of page indices
        let pages = (addr as u64 / self.page_size as u64) as usize .. (end_addr / self.page_size as u64) as usize + 1;
        let pages_len = pages.len();
        
        let mut buf = Cursor::new(buf);

        for (i, page) in pages.enumerate() {
            let start = if i > 0 { 0 } else { addr as usize % self.page_size };
            let end = if i < pages_len - 1 { self.page_size } else { (end_addr as usize % self.page_size) + 1 };

            if let Some(page) = self.page(page % self.page_count()) {
                buf.write_all(&page[start .. end]).unwrap();
            } else {
                // Page not allocated, fill `buf` with 0s

                let count = (end - start) + 1;
                let position = buf.position();

                unsafe { ptr::write_bytes(buf.get_mut().as_mut_ptr(), 0, count); }
                buf.set_position(position + count as u64);
            }
        }
    }

    /// Writes bytes from the specified buffer `buf` starting at byte address `addr`.
    pub fn write(&mut self, addr: u32, buf: &[u8]) {
        let end_addr = addr as u64 + (buf.len() as u64).saturating_sub(1);

        if end_addr == addr as u64 {
            return;
        }

        // Writes may span multiple pages, so we get a range of page indices
        let pages = (addr as u64 / self.page_size as u64) as usize .. (end_addr / self.page_size as u64) as usize + 1;
        let pages_len = pages.len();

        let mut buf = Cursor::new(buf);

        for (i, page) in pages.enumerate() {
            let start = if i > 0 { 0 } else { addr as usize % self.page_size };
            let end = if i < pages_len - 1 { self.page_size } else { (end_addr as usize % self.page_size) + 1 };

            let page = {
                // Get `page_count` first to avoid current limitations in borrowck
                let page_count = self.page_count();
                self.page_mut(page % page_count)
            };
            buf.read_exact(&mut page[start .. end]).unwrap();
        }
    }

    /// Reads an unsigned 32 bit integer starting at byte address `addr`.
    #[inline]
    pub fn read_u32(&self, addr: u32) -> u32 {
        let mut buf = [0u8; 4];

        self.read(addr, &mut buf);
        LittleEndian::read_u32(&buf)
    }

    /// Writes an unsigned 32 bit integer starting at byte address `addr`.
    #[inline]
    pub fn write_u32(&mut self, addr: u32, value: u32) {
        let mut buf = [0u8; 4];

        LittleEndian::write_u32(&mut buf, value);
        self.write(addr, &buf);
    }

    // #[inline]
    // pub fn pop(&mut self) -> Result<u32, Error> {
    //     self.stack_ptr = self.stack_ptr.wrapping_add(4);
    //     self.read_u32(self.stack_ptr)
    // }

    // #[inline]
    // pub fn push(&mut self, val: u32) -> Result<(), Error> {
    //     let stack_ptr = self.stack_ptr;
    //     self.stack_ptr = self.stack_ptr.wrapping_sub(4);
        
    //     self.write_u32(stack_ptr, val)
    // }
}

#[cfg(test)]
mod test {
    use super::{Memory, DEFAULT_PAGE_SIZE as PAGE_SIZE};

    const PAGE_COUNT: usize = ((1u64 << 32) / PAGE_SIZE as u64) as usize;

    #[test]
    fn read() {
        let mut mem = Memory::new();
        mem.page_mut(0)[..4].copy_from_slice(&[0xff; 4]);

        let mut buf = [0; 4];

        mem.read(0, &mut buf);
        assert_eq!(buf, [0xff; 4]);
    }

    #[test]
    fn write() {
        let mut mem = Memory::new();
        mem.page_mut(0)[..4].copy_from_slice(&[0xff; 4]);

        let buf = [0; 4];

        mem.write(0, &buf);
        assert_eq!(mem.page(0).unwrap()[..4], [0; 4]);
    }

    #[test]
    fn read_unmapped() {
        let mem = Memory::new();
        let mut buf = [0xff; 4];
        
        mem.read(0, &mut buf);
        assert_eq!(buf, [0; 4]);
    }

    #[test]
    fn write_unmapped() {
        let mut mem = Memory::new();
        let buf = [0xff; 4];

        mem.write(0, &buf);
        assert_eq!(mem.page(0).unwrap()[..4], [0xff; 4]);
    }

    #[test]
    fn read_boundary() {
        let mut mem = Memory::new();
        mem.page_mut(0)[(PAGE_SIZE - 2)..].copy_from_slice(&[0xff; 2]);
        mem.page_mut(1)[..2].copy_from_slice(&[0xff; 2]);

        let mut buf = [0; 4];

        mem.read(PAGE_SIZE as u32 - 2, &mut buf);
        assert_eq!(buf, [0xff; 4]);
    }

    #[test]
    fn write_boundary() {
        let mut mem = Memory::new();
        mem.page_mut(0)[(PAGE_SIZE - 2)..].copy_from_slice(&[0xff; 2]);
        mem.page_mut(1)[..2].copy_from_slice(&[0xff; 2]);

        let buf = [0; 4];

        mem.write(PAGE_SIZE as u32 - 2, &buf);
        assert_eq!(mem.page(0).unwrap()[(PAGE_SIZE - 2)..], [0; 2]);
        assert_eq!(mem.page(1).unwrap()[..2], [0; 2]);
    }

    #[test]
    fn read_wrapping() {
        let mut mem = Memory::new();
        mem.page_mut(0)[..2].copy_from_slice(&[0xff; 2]);
        mem.page_mut(PAGE_COUNT - 1)[(PAGE_SIZE - 2)..].copy_from_slice(&[0xff; 2]);

        let mut buf = [0; 4];

        mem.read(u32::max_value() - 1, &mut buf);
        assert_eq!(buf, [0xff; 4]);
    }

    #[test]
    fn write_wrapping() {
        let mut mem = Memory::new();
        mem.page_mut(0)[..2].copy_from_slice(&[0xff; 2]);
        mem.page_mut(PAGE_COUNT - 1)[(PAGE_SIZE - 2)..].copy_from_slice(&[0xff; 2]);

        let buf = [0; 4];

        mem.write(u32::max_value() - 1, &buf);
        assert_eq!(mem.page(0).unwrap()[..2], [0; 2]);
        assert_eq!(mem.page(PAGE_COUNT - 1).unwrap()[PAGE_SIZE - 2 ..], [0; 2]);
    }
}
