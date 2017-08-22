use std::convert::TryInto;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::mem;
use std::ops::{BitAnd, BitOr};

use enum_traits::BitPattern;

use vec_map::VecMap;

use {Error, Instruction, Memory};

pub struct VirtualMachine {
    pub memory: Memory,
    pub registers: [u32; 32],
    pub breakpoints_enabled: bool,
    pub verbose_output: bool,
    file_handles: VecMap<File>
}

impl Default for VirtualMachine {
    fn default() -> Self {
        Self::new(Vec::new()).unwrap()
    }
}

impl VirtualMachine {
    pub fn new(program: Vec<u8>) -> Result<Self, Error> {
        if program.len() as u64 > u32::max_value() as u64 + 1 {
            return Err(Error::ProgramTooLarge);
        }

        let mut vm = Self {
            memory: Memory::default(),
            registers: [0; 32],
            breakpoints_enabled: false,
            verbose_output: false,
            file_handles: VecMap::new()
        };
        vm.reset();
        vm.memory.write(0, &program);

        Ok(vm)
    }

    pub fn with_page_size(page_size: usize, program: Vec<u8>) -> Result<Self, Error> {
        if program.len() as u64 > u32::max_value() as u64 + 1 {
            return Err(Error::ProgramTooLarge);
        }

        let mut vm = Self {
            memory: Memory::with_page_size(page_size),
            registers: [0; 32],
            breakpoints_enabled: false,
            verbose_output: false,
            file_handles: VecMap::new()
        };
        vm.reset();
        vm.memory.write(0, &program);

        Ok(vm)
    }

    #[inline]
    pub fn program_ctr(&self) -> u32 {
        self.registers[0]
    }

    #[inline]
    pub fn program_ctr_mut(&mut self) -> &mut u32 {
        &mut self.registers[0]
    }

    #[inline]
    pub fn stack_ptr(&self) -> u32 {
        self.registers[1]
    }

    #[inline]
    pub fn stack_ptr_mut(&mut self) -> &mut u32 {
        &mut self.registers[1]
    }

    #[inline]
    pub fn reset(&mut self) {
        *self.program_ctr_mut() = 0;
        *self.stack_ptr_mut() = 0xfffffffc;
    }

    pub fn run(&mut self) -> Result<i32, Error> {
        loop {
            let instr = self.memory.read_u32(self.program_ctr()).try_into()?;
            
            match self.exec_instr(instr)? {
                Some(status) => return Ok(status),
                _ => {}
            }
        }
    }

    fn exec_instr(&mut self, instr: Instruction) -> Result<Option<i32>, Error> {
        use OpCode::*;

        if self.verbose_output {
            println!("{:?}", instr);
        }

        *self.program_ctr_mut() += instr.size();

        match instr {
            Instruction::Register { op, dst, src1, src2 } => {
                let src1 = self.registers[src1];
                let src2 = self.registers[src2];

                self.registers[dst] = match op {
                    ADD | C_ADD => src1.wrapping_add(src2),
                    SUB | C_SUB => src1.wrapping_sub(src2),
                    AND | C_AND => src1 & src2,
                    OR  | C_OR  => src1 | src2,
                    XOR | C_XOR => src1 ^ src2,
                    SLL | C_SLL => src1 << (src2 & 0x1f),
                    SRL | C_SRL => src1 >> (src2 & 0x1f),
                    SRA | C_SRA => ((src1 as i32) >> (src2 & 0x1f)) as u32,
                    MV => src2,
                    _  => unreachable!("{:?}", op)
                };
            },
            Instruction::Immediate { op, dst, src1, imm } => {
                let src1 = self.registers[src1];

                self.registers[dst] = match op {
                    ADDI | C_ADDI => (src1 as i32).wrapping_add(imm as i32) as u32,
                    ANDI | C_ANDI => src1 & imm,
                    ORI  | C_ORI  => src1 | imm,
                    XORI | C_XORI => src1 ^ imm,
                    SLLI | C_SLLI => src1 << (imm & 0x1f),
                    SRLI | C_SRLI => src1 >> (imm & 0x1f),
                    SRAI | C_SRAI => ((src1 as i32) >> (imm & 0x1f)) as u32,
                    LI   | C_LI   => imm,
                    BEZ | C_BEZ => {
                        if src1 == 0 {
                            self.registers[0] = (self.registers[0] as i32).wrapping_add(imm as i32) as u32
                        }
                        return Ok(None);
                    },
                    BNZ | C_BNZ => {
                        if src1 != 0 {
                            self.registers[0] = (self.registers[0] as i32).wrapping_add(imm as i32) as u32
                        }
                        return Ok(None);
                    },
                    LOAD | C_LOAD => self.memory.read_u32((src1 as i32).wrapping_add(imm as i32) as u32),
                    CALL | C_CALL => return self.exec_syscall(imm as u16),
                    BREAK | C_BREAK => {
                        if self.breakpoints_enabled {
                            println!("breakpoint:\n\tr0 (pc): {}, r1 (sp): 0x{:x}, r2 (lr): {}, r3 (rv): {}",
                                self.registers[0], self.registers[1], self.registers[2], self.registers[3]);
                            println!("\tr4: {}, r5: {}, r6: {}, r7: {}\nPress enter to continue...",
                                self.registers[4], self.registers[5], self.registers[6], self.registers[7]);

                            // Wait for user input
                            io::stdin().read_line(&mut String::new()).ok();
                        }

                        return Ok(None);
                    },
                    _ => unreachable!()
                };
            },
            Instruction::Store { op, src1, src2, imm } => {
                let src1 = self.registers[src1];
                let src2 = self.registers[src2];

                let program_ctr = self.registers[0] as i32;

                match op {
                    STORE | C_STORE => self.memory.write_u32((src1 as i32).wrapping_add(imm as i32) as u32, src2),
                    BEQ => if src1 == src2 { self.registers[0] = program_ctr.wrapping_add(imm as i32) as u32 },
                    BNE => if src1 != src2 { self.registers[0] = program_ctr.wrapping_add(imm as i32) as u32 },
                    BLT => if (src1 as i32) < (src2 as i32) {
                        self.registers[0] = program_ctr.wrapping_add(imm as i32) as u32
                    },
                    BGE => if (src1 as i32) >= (src2 as i32) {
                        self.registers[0] = program_ctr.wrapping_add(imm as i32) as u32
                    },
                    BLT_U => if src1 < src2 { self.registers[0] = program_ctr.wrapping_add(imm as i32) as u32 },
                    BGE_U => if src1 >= src2 { self.registers[0] = program_ctr.wrapping_add(imm as i32) as u32 },
                    _ => unreachable!()
                }
            },
            Instruction::Upper { op, dst, imm } => {
                self.registers[dst] = match op {
                    LUI | C_LUI => imm,
                    _ => unreachable!()
                };
            }
        }

        Ok(None)
    }

    fn exec_syscall(&mut self, call: u16) -> Result<Option<i32>, Error> {
        if self.verbose_output {
            println!("syscall: {}", call);
        }

        match call {
            0 => { // sys_exit
                return Ok(Some(self.registers[4] as i32));
            },
            1 => { // sys_read
                let handle = self.registers[4];
                let ptr = self.registers[5];
                let len = self.registers[6];

                self.registers[3] = self.read_file(handle, ptr, len)
                                        .unwrap_or_else(|e| { println!("{}", e); -1i32 as u32 });
            },
            2 => { // sys_write
                let handle = self.registers[4];
                let ptr = self.registers[5];
                let len = self.registers[6];

                self.registers[3] = self.write_file(handle, ptr, len)
                                        .unwrap_or_else(|e| { println!("{}", e); -1i32 as u32 });
            },
            3 => { // sys_open
                let ptr = self.registers[4];
                let len = self.registers[5];
                let flags = self.registers[6];

                self.registers[3] = self.open_file(ptr, len, flags)
                                        .unwrap_or_else(|e| { println!("{}", e); -1i32 as u32 });
            },
            4 => { // sys_close
                let handle = self.registers[4];

                self.registers[3] = self.close_file(handle).unwrap_or_else(|e| { println!("{}", e); -1i32 as u32 });
            },
            5 => { // sys_create
                let ptr = self.registers[4];
                let len = self.registers[5];

                self.registers[3] = self.create_file(ptr, len).unwrap_or_else(|e| { println!("{}", e); -1i32 as u32 });
            },
            _ => return Err(Error::InvalidSysCall(call))
        }

        Ok(None)
    }

    fn read_file(&mut self, handle: u32, ptr: u32, len: u32) -> Result<u32, io::Error> {
        let mut buf = vec![0; len as usize];
        
        let i = match handle {
            0 => io::stdin().read(&mut buf)?,
            1 | 2 => return Ok(-1i32 as u32),
            d @ _ => match self.file_handles.get_mut(d as usize - 3) {
                Some(ref mut file) => file.read(&mut buf)?,
                None => return Ok(-1i32 as u32)
            }
        };

        self.memory.write(ptr, &buf[..i]);
        Ok(i as u32)
    }

    fn write_file(&mut self, handle: u32, ptr: u32, len: u32) -> Result<u32, io::Error> {
        let mut buf = vec![0; len as usize];
        self.memory.read(ptr, &mut buf);

        let i = match handle {
            0 => return Ok(-1i32 as u32),
            1 => io::stdout().write(&buf)?,
            2 => io::stderr().write(&buf)?,
            d @ _ => match self.file_handles.get_mut(d as usize - 3) {
                Some(ref mut file) => file.write(&buf)?,
                None => return Ok(-1i32 as u32)
            }
        };

        Ok(i as u32)
    }

    fn open_file(&mut self, ptr: u32, len: u32, flags: u32) -> Result<u32, io::Error> {
        let mut buf = vec![0; len as usize];
        self.memory.read(ptr, &mut buf);

        let path = String::from_utf8_lossy(&buf);
        let mut options = OpenOptions::new();

        macro_rules! apply_flags {
            ($($flag:ident, $func:ident),+) => {
                $(if flags & FileFlags::$flag != 0 { options.$func(true); })+
            }
        }

        apply_flags!(
            READ, read,
            WRITE, write,
            CREATE, create,
            EXCLUSIVE, create_new,
            TRUNCATE, truncate,
            APPEND, append
        );

        let file = options.open(&path.as_ref())?;

        for handle in 0..(u32::max_value() as usize - 2) {
            if !self.file_handles.contains_key(handle) {
                self.file_handles.insert(handle, file);
                return Ok(handle as u32 + 3);
            }
        }

        Ok(-1i32 as u32)
    }

    fn close_file(&mut self, handle: u32) -> Result<u32, io::Error> {
        if let Some(file) = self.file_handles.remove(handle as usize - 3) {
            file.sync_all()?;

            // Since the file has been moved out of the VecMap, it will get
            // closed once `file` is dropped when we return
            return Ok(0);
        }

        Ok(-1i32 as u32)
    }

    fn create_file(&mut self, ptr: u32, len: u32) -> Result<u32, io::Error> {
        self.open_file(ptr, len, FileFlags::CREATE | FileFlags::WRITE | FileFlags::TRUNCATE)
    }
}

#[repr(u32)]
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, EnumBitPattern)]
enum FileFlags {
    READ,
    WRITE,
    CREATE,
    EXCLUSIVE,
    TRUNCATE,
    APPEND
}

impl BitAnd<FileFlags> for u32 {
    type Output = u32;

    fn bitand(self, rhs: FileFlags) -> Self::Output {
        // `#[derive(EnumBitPattern)]` sets the return type of `bit_pattern()`
        // to the shortest byte array that will fit all the enum variants,
        // which in this case is `[u8; 1]`, hence the transmute to `u8`.
        let rhs: u8 = unsafe { mem::transmute(rhs.bit_pattern()) };
        
        self & rhs as u32
    }
}

impl BitOr for FileFlags {
    type Output = u32;

    fn bitor(self, rhs: Self) -> Self::Output {
        // See comment in `bitand()` above
        let lhs: u8 = unsafe { mem::transmute(self.bit_pattern()) };
        let rhs: u8 = unsafe { mem::transmute(rhs.bit_pattern()) };

        (lhs | rhs) as u32
    }
}

impl BitOr<FileFlags> for u32 {
    type Output = u32;

    fn bitor(self, rhs: FileFlags) -> Self::Output {
        // See comment in `bitand()` above
        let rhs: u8 = unsafe { mem::transmute(rhs.bit_pattern()) };

        self | rhs as u32
    }
}

#[cfg(test)]
mod test {
    use std::fs::{self, File};
    use std::io::{Read, Write};
    use std::path::Path;

    use Instruction::*;
    use OpCode::*;

    use super::VirtualMachine;

    #[test]
    fn add() {
        let instr = Register { op: ADD, dst: 2, src1: 0, src2: 0 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 8);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn sub() {
        let instr = Register { op: SUB, dst: 2, src1: 0, src2: 2 };
        let mut vm = VirtualMachine::default();
        vm.registers[2] = 3;

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 1);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn and() {
        let instr = Register { op: AND, dst: 2, src1: 2, src2: 3 };
        let mut vm = VirtualMachine::default();
        vm.registers[2] = 0b10101010;
        vm.registers[3] = 0b11010100;

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..4], [0b10000000, 0b11010100]);
        assert_eq!(vm.registers[4..], [0; 28]);
    }

    #[test]
    fn or() {
        let instr = Register { op: OR, dst: 2, src1: 2, src2: 3 };
        let mut vm = VirtualMachine::default();
        vm.registers[2] = 0b10101010;
        vm.registers[3] = 0b11010100;

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..4], [0b11111110, 0b11010100]);
        assert_eq!(vm.registers[4..], [0; 28]);
    }

    #[test]
    fn xor() {
        let instr = Register { op: XOR, dst: 2, src1: 2, src2: 3 };
        let mut vm = VirtualMachine::default();
        vm.registers[2] = 0b10101010;
        vm.registers[3] = 0b11010100;

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..4], [0b01111110, 0b11010100]);
        assert_eq!(vm.registers[4..], [0; 28]);
    }

    #[test]
    fn sll() {
        let instr = Register { op: SLL, dst: 2, src1: 0, src2: 0 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 64);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn srl() {
        let instr = Register { op: SRL, dst: 2, src1: 0, src2: 2 };
        let mut vm = VirtualMachine::default();
        vm.registers[2] = 2;

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 1);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn sra() {
        let instr = Register { op: SRA, dst: 2, src1: 1, src2: 2 };
        let mut vm = VirtualMachine::default();
        vm.registers[2] = 2;

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 0xffffffff);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn addi() {
        let instr = Immediate { op: ADDI, dst: 2, src1: 0, imm: 2 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 6);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn andi() {
        let instr = Immediate { op: ANDI, dst: 2, src1: 2, imm: 0b11010100 };
        let mut vm = VirtualMachine::default();
        vm.registers[2] = 0b10101010;

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 0b10000000);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn ori() {
        let instr = Immediate { op: ORI, dst: 2, src1: 2, imm: 0b11010100 };
        let mut vm = VirtualMachine::default();
        vm.registers[2] = 0b10101010;

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 0b11111110);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn xori() {
        let instr = Immediate { op: XORI, dst: 2, src1: 2, imm: 0b11010100 };
        let mut vm = VirtualMachine::default();
        vm.registers[2] = 0b10101010;

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 0b01111110);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn slli() {
        let instr = Immediate { op: SLLI, dst: 2, src1: 0, imm: 4 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 64);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn srli() {
        let instr = Immediate { op: SRLI, dst: 2, src1: 0, imm: 2 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 1);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn srai() {
        let instr = Immediate { op: SRAI, dst: 2, src1: 1, imm: 2 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 0xffffffff);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn bez() {
        let instr = Immediate { op: BEZ, dst: 2, src1: 2, imm: 4 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 8);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..], [0; 30]);
    }

    #[test]
    fn bnz() {
        let instr = Immediate { op: BNZ, dst: 1, src1: 1, imm: 4 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 8);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..], [0; 30]);
    }

    #[test]
    fn beq() {
        let instr = Store { op: BEQ, src1: 1, src2: 1, imm: 4 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 8);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..], [0; 30]);
    }

    #[test]
    fn bne() {
        let instr = Store { op: BNE, src1: 0, src2: 1, imm: 4 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 8);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..], [0; 30]);
    }

    #[test]
    fn blt() {
        let instr = Store { op: BLT, src1: 1, src2: 0, imm: 4 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 8);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..], [0; 30]);
    }

    #[test]
    fn bge() {
        let instr = Store { op: BGE, src1: 0, src2: 1, imm: 4 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 8);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..], [0; 30]);
    }

    #[test]
    fn blt_u() {
        let instr = Store { op: BLT_U, src1: 0, src2: 1, imm: 4 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 8);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..], [0; 30]);
    }

    #[test]
    fn bge_u() {
        let instr = Store { op: BGE_U, src1: 1, src2: 0, imm: 4 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 8);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..], [0; 30]);
    }

    #[test]
    fn li() {
        let instr = Immediate { op: LI, dst: 2, src1: 2, imm: 1 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 1);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn lui() {
        let instr = Upper { op: LUI, dst: 2, imm: 0xffff0000 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 0xffff0000);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn load() {
        let instr = Immediate { op: LOAD, dst: 2, src1: 0, imm: 2 };
        let mut vm = VirtualMachine::new(vec![0, 0, 0, 0, 0, 0, 0x1f, 0x2c]).unwrap();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 0x2c1f);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn store() {
        let instr = Store { op: STORE, src1: 0, src2: 0, imm: 2 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..], [0; 30]);
        assert_eq!(vm.memory.read_u32(6), 4);
    }

    #[test]
    fn break_() {
        let instr = Immediate { op: BREAK, dst: 0, src1: 0, imm: 0 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..], [0; 30]);
    }

    #[test]
    fn mv() {
        let instr = Register { op: MV, dst: 2, src1: 2, src2: 0 };
        let mut vm = VirtualMachine::default();

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 2);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2], 2);
        assert_eq!(vm.registers[3..], [0; 29]);
    }

    #[test]
    fn sys_exit() {
        let instr = Immediate { op: CALL, dst: 0, src1: 0, imm: 0 };
        let mut vm = VirtualMachine::default();
        vm.registers[4] = 1;

        assert_eq!(vm.exec_instr(instr), Ok(Some(1)));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..5], [0, 0, 1]);
        assert_eq!(vm.registers[5..], [0; 27]);
    }

    #[test]
    fn sys_read() {
        let path = Path::new(".sys_read_test");
        let file = {
            {
                let mut file = File::create(&path).unwrap();
                file.write(b"Hello, World!").unwrap();
                file.flush().unwrap();
            }

            File::open(&path).unwrap()
        };

        let instr = Immediate { op: CALL, dst: 0, src1: 0, imm: 1 };
        let mut vm = VirtualMachine::default();
        vm.registers[4..7].copy_from_slice(&[3, 0, 13]);
        vm.file_handles.insert(0, file);

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..7], [0, 13, 3, 0, 13]);
        assert_eq!(vm.registers[7..], [0; 25]);
        
        let mut buf = [0; 13];
        vm.memory.read(0, &mut buf);

        assert_eq!(&buf, b"Hello, World!");

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn sys_write() {
        let path = Path::new(".sys_write_test");
        let file = File::create(&path).unwrap();

        let instr = Immediate { op: CALL, dst: 0, src1: 0, imm: 2 };
        let mut vm = VirtualMachine::default();
        vm.registers[4..7].copy_from_slice(&[3, 0, 13]);
        vm.memory.write(0, b"Hello, World!");
        vm.file_handles.insert(0, file);

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..7], [0, 13, 3, 0, 13]);
        assert_eq!(vm.registers[7..], [0; 25]);
        
        vm.file_handles.remove(0).unwrap().flush().unwrap();
        let mut file = File::open(&path).unwrap();
        let mut buf = [0; 13];
        file.read(&mut buf).unwrap();

        assert_eq!(&buf, b"Hello, World!");

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn sys_open() {
        let path = Path::new(".sys_open_test");
        {
            File::create(&path).unwrap();
        }

        let instr = Immediate { op: CALL, dst: 0, src1: 0, imm: 3 };
        let mut vm = VirtualMachine::default();
        vm.registers[4..7].copy_from_slice(&[0, 14, 1]);
        vm.memory.write(0, b".sys_open_test");

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..7], [0, 3, 0, 14, 1]);
        assert_eq!(vm.registers[7..], [0; 25]);
        assert_eq!(vm.file_handles.remove(0).is_some(), true);

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn sys_close() {
        let path = Path::new(".sys_close_test");
        let file = File::create(&path).unwrap();

        let instr = Immediate { op: CALL, dst: 0, src1: 0, imm: 4 };
        let mut vm = VirtualMachine::default();
        vm.registers[4] = 3;
        vm.file_handles.insert(0, file);

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..5], [0, 0, 3]);
        assert_eq!(vm.registers[5..], [0; 27]);
        assert_eq!(vm.file_handles.get(0).is_none(), true);

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn sys_create() {
        let path = Path::new(".sys_create_test");

        let instr = Immediate { op: CALL, dst: 0, src1: 0, imm: 5 };
        let mut vm = VirtualMachine::default();
        vm.registers[4..6].copy_from_slice(&[0, 16]);
        vm.memory.write(0, b".sys_create_test");

        assert_eq!(vm.exec_instr(instr), Ok(None));

        assert_eq!(vm.program_ctr(), 4);
        assert_eq!(vm.stack_ptr(), 0xfffffffc);
        assert_eq!(vm.registers[2..6], [0, 3, 0, 16]);
        assert_eq!(vm.registers[6..], [0; 26]);
        assert_eq!(vm.file_handles.remove(0).is_some(), true);

        fs::remove_file(&path).unwrap();
    }
}
