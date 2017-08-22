use std::convert::TryFrom;
use std::io;

use byteorder::{LittleEndian, WriteBytesExt};

use enum_traits::Discriminant;

use Error;

const OP_CODE_MASK: u32 = (1 << 6) - 1;

const DST_REG_SHIFT: u32 = 6;
const DST_REG_MASK: u32 = ((1 << 5) - 1) << DST_REG_SHIFT;

const SRC1_REG_SHIFT: u32 = 11;
const SRC1_REG_MASK: u32 = ((1 << 5) - 1) << SRC1_REG_SHIFT;

const SRC2_REG_SHIFT: u32 = 16;
const SRC2_REG_MASK: u32 = ((1 << 5) - 1) << SRC2_REG_SHIFT;

const IMM_SHIFT: u32 = 16;
const IMM_MASK: u32 = ((1 << 16) - 1) << IMM_SHIFT;

const IMM_STORE_SHIFT1: u32 = 6;
const IMM_STORE_MASK1: u32 = ((1 << 5) - 1) << IMM_STORE_SHIFT1;

const IMM_STORE_SHIFT2: u32 = 16;
const IMM_STORE_MASK2: u32 = ((1 << 11) - 1) << (IMM_STORE_SHIFT2 + 5);

const C_DST1_REG_SHIFT: u32 = 6;
const C_DST1_REG_MASK: u32 = ((1 << 3) - 1) << C_DST1_REG_SHIFT;

const C_DST2_REG_SHIFT: u32 = 6;
const C_DST2_REG_MASK: u32 = ((1 << 2) - 1) << C_DST2_REG_SHIFT;

const C_SRC1_REG_SHIFT: u32 = 11;
const C_SRC1_REG_MASK: u32 = ((1 << 2) - 1) << C_SRC1_REG_SHIFT;

const C_IMM_SHIFT: u32 = 9;
const C_IMM_MASK: u32 = ((1 << 7) - 1) << C_IMM_SHIFT;

const C_IMM_UPPER_SHIFT: u32 = 7;

const C_IMM_STORE_SHIFT1: u32 = 7;
const C_IMM_STORE_MASK1: u32 = ((1 << 3) - 1) << (C_IMM_STORE_SHIFT1 + 1);

const C_IMM_STORE_SHIFT2: u32 = 9;
const C_IMM_STORE_MASK2: u32 = ((1 << 3) - 1) << (C_IMM_STORE_SHIFT2 + 4);

#[repr(u32)]
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, EnumDiscriminant, EnumFromVariantName, Eq, PartialEq)]
pub enum OpCode {
    // 0x00 reserved as invalid instruction

    ADD = 0x02,
    SUB = 0x04,
    AND = 0x06,
    OR  = 0x08,
    XOR = 0x0a,
    SLL = 0x0c,
    SRL = 0x0e,
    SRA = 0x10,

    ADDI = 0x12,
    ANDI = 0x14,
    ORI  = 0x16,
    XORI = 0x18,
    SLLI = 0x1a,
    SRLI = 0x1c,
    SRAI = 0x1e,

    BEZ = 0x20,
    BNZ = 0x22,
    BEQ = 0x24,
    BNE = 0x26,
    BLT = 0x28,
    BGE = 0x2a,
    BLT_U = 0x2c,
    BGE_U = 0x2e,

    LI  = 0x30,
    LUI = 0x32,

    LOAD  = 0x34,
    STORE = 0x36,

    // 0x38, 0x3a reserved

    CALL  = 0x3c,
    BREAK = 0x3e,

    // 0x01 reserved as invalid instruction

    C_ADD = 0x03,
    C_SUB = 0x05,
    C_AND = 0x07,
    C_OR  = 0x09,
    C_XOR = 0x0b,
    C_SLL = 0x0d,
    C_SRL = 0x0f,
    C_SRA = 0x11,

    C_ADDI = 0x13,
    C_ANDI = 0x15,
    C_ORI  = 0x17,
    C_XORI = 0x19,
    C_SLLI = 0x1b,
    C_SRLI = 0x1d,
    C_SRAI = 0x1f,

    C_BEZ = 0x21,
    C_BNZ = 0x23,

    // 0x25, 0x27, 0x29, 0x2b, 0x2d, 0x2f reserved

    C_LI  = 0x31,
    C_LUI = 0x33,

    C_LOAD  = 0x35,
    C_STORE = 0x37,

    MV = 0x39,

    // 0x3b reserved

    C_CALL  = 0x3d,
    C_BREAK = 0x3f,
}

/// Representation of a decoded instruction.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Instruction {
    /// R-Type and CR-Type instructions
    Register { op: OpCode, dst: usize, src1: usize, src2: usize },

    /// I-Type, CI-Type and CL-Type instructions
    Immediate { op: OpCode, dst: usize, src1: usize, imm: u32 },

    /// S-Type and CS-Type instructions
    Store { op: OpCode, src1: usize, src2: usize, imm: u32 },

    /// U-Type and CU-Type instructions
    Upper { op: OpCode, dst: usize, imm: u32 }
}

impl Instruction {
    /// Returns the size of this instruction in bytes.
    pub fn size(&self) -> u32 {
        use Instruction::*;

        match *self {
            Register { op, .. } | Immediate { op, .. } | Store { op, .. } | Upper { op, .. } => {
                if (op as u32) & 1 == 0 { 4 } else { 2 }
            }
        }
    }

    pub fn write_bytes(&self, buf: &mut Vec<u8>) -> Result<(), io::Error> {
        use Instruction::*;

        match self.size() {
            2 => {
                let instr = match *self {
                    Register { op, dst, src2, .. } => {
                        ((op as u32) & OP_CODE_MASK) |
                        (((dst as u32) << DST_REG_SHIFT) & DST_REG_MASK) |
                        (((src2 as u32) << SRC1_REG_SHIFT) & SRC1_REG_MASK)
                    },
                    Immediate { op, dst, src1, imm } => {
                        if op == OpCode::C_LOAD {
                            ((op as u32) & OP_CODE_MASK) |
                            (((dst as u32) << C_DST2_REG_SHIFT) & C_DST2_REG_MASK) |
                            (((src1 as u32) << C_SRC1_REG_SHIFT) & C_SRC1_REG_MASK) |
                            ((imm << C_IMM_STORE_SHIFT1) & C_IMM_STORE_MASK1) |
                            ((imm << C_IMM_STORE_SHIFT2) & C_IMM_STORE_MASK2)
                        } else {
                            ((op as u32) & OP_CODE_MASK) |
                            (((dst as u32) << C_DST1_REG_SHIFT) & C_DST1_REG_MASK) |
                            ((imm << C_IMM_SHIFT) & C_IMM_MASK)
                        }
                    },
                    Store { op, src1, src2, imm } => {
                        ((op as u32) & OP_CODE_MASK) |
                        (((src1 as u32) << C_DST2_REG_SHIFT) & C_DST2_REG_MASK) |
                        (((src2 as u32) << C_SRC1_REG_SHIFT) & C_SRC1_REG_MASK) |
                        ((imm << C_IMM_STORE_SHIFT1) & C_IMM_STORE_MASK1) |
                        ((imm << C_IMM_STORE_SHIFT2) & C_IMM_STORE_MASK2)
                    },
                    Upper { op, dst, imm } => {
                        ((op as u32) & OP_CODE_MASK) |
                        (((dst as u32) << C_DST1_REG_SHIFT) & C_DST1_REG_MASK) |
                        ((imm >> C_IMM_UPPER_SHIFT) & C_IMM_MASK)
                    }
                };

                buf.write_u16::<LittleEndian>(instr as u16)
            },
            4 => {
                let instr = match *self {
                    Register { op, dst, src1, src2 } => {
                        ((op as u32) & OP_CODE_MASK) |
                        (((dst as u32) << DST_REG_SHIFT) & DST_REG_MASK) |
                        (((src1 as u32) << SRC1_REG_SHIFT) & SRC1_REG_MASK) |
                        (((src2 as u32) << SRC2_REG_SHIFT) & SRC2_REG_MASK)
                    },
                    Immediate { op, dst, src1, imm } => {
                        ((op as u32) & OP_CODE_MASK) |
                        (((dst as u32) << DST_REG_SHIFT) & DST_REG_MASK) |
                        (((src1 as u32) << SRC1_REG_SHIFT) & SRC1_REG_MASK) |
                        ((imm << IMM_SHIFT) & IMM_MASK)
                    },
                    Store { op, src1, src2, imm } => {
                        ((op as u32) & OP_CODE_MASK) |
                        (((src1 as u32) << SRC1_REG_SHIFT) & SRC1_REG_MASK) |
                        (((src2 as u32) << SRC2_REG_SHIFT) & SRC2_REG_MASK) |
                        ((imm << IMM_STORE_SHIFT1) & IMM_STORE_MASK1) |
                        ((imm << IMM_STORE_SHIFT2) & IMM_STORE_MASK2)
                    },
                    Upper { op, dst, imm } => {
                        ((op as u32) & OP_CODE_MASK) |
                        (((dst as u32) << DST_REG_SHIFT) & DST_REG_MASK) |
                        (imm & IMM_MASK)
                    }
                };

                buf.write_u32::<LittleEndian>(instr)
            },
            _ => unreachable!()
        }
    }
}

impl TryFrom<u32> for Instruction {
    type Error = Error;

    fn try_from(instr: u32) -> Result<Self, Self::Error> {
        use Instruction::*;
        use OpCode::*;

        let op = {
            let op = instr & OP_CODE_MASK;
            OpCode::from_discriminant(op).ok_or(Error::InvalidOpCode(op))?
        };

        Ok(match op {
            ADD | SUB | AND | OR | XOR | SLL | SRL | SRA => Register {
                op: op,
                dst: ((instr & DST_REG_MASK) >> DST_REG_SHIFT) as usize,
                src1: ((instr & SRC1_REG_MASK) >> SRC1_REG_SHIFT) as usize,
                src2: ((instr & SRC2_REG_MASK) >> SRC2_REG_SHIFT) as usize
            },
            ADDI | ANDI | ORI | XORI | SLLI | SRLI | SRAI | BEZ | BNZ | LI | LOAD | CALL | BREAK => Immediate {
                op: op,
                dst: ((instr & DST_REG_MASK) >> DST_REG_SHIFT) as usize,
                src1: ((instr & SRC1_REG_MASK) >> SRC1_REG_SHIFT) as usize,
                imm: ((instr & IMM_MASK) as i32 >> IMM_SHIFT as i32) as u32
            },
            STORE | BEQ | BNE | BGE | BLT | BGE_U | BLT_U => Store {
                op: op,
                src1: ((instr & SRC1_REG_MASK) >> SRC1_REG_SHIFT) as usize,
                src2: ((instr & SRC2_REG_MASK) >> SRC2_REG_SHIFT) as usize,
                imm: ((instr & IMM_STORE_MASK1) >> IMM_STORE_SHIFT1) |
                     ((instr & IMM_STORE_MASK2) as i32 >> IMM_STORE_SHIFT2 as i32) as u32
            },
            LUI => Upper {
                op: op,
                dst: ((instr & DST_REG_MASK) >> DST_REG_SHIFT) as usize,
                imm: instr & IMM_MASK
            },
            C_ADD | C_SUB | C_AND | C_OR | C_XOR | C_SLL | C_SRL | C_SRA | MV => {
                Register {
                    op: op,
                    dst: ((instr & DST_REG_MASK) >> DST_REG_SHIFT) as usize,
                    src1: ((instr & DST_REG_MASK) >> DST_REG_SHIFT) as usize,
                    src2: ((instr & SRC1_REG_MASK) >> SRC1_REG_SHIFT) as usize
                }
            },
            C_ADDI | C_ANDI | C_ORI | C_XORI | C_SLLI | C_SRLI | C_SRAI |
            C_BEZ | C_BNZ | C_LI | C_CALL | C_BREAK => Immediate {
                op: op,
                dst: ((instr & C_DST1_REG_MASK) >> C_DST1_REG_SHIFT) as usize,
                src1: ((instr & C_DST1_REG_MASK) >> C_DST1_REG_SHIFT) as usize,
                imm: (((instr & C_IMM_MASK) << 16) as i32 >> (C_IMM_SHIFT as i32 + 16)) as u32
            },
            C_LOAD => Immediate {
                op: op,
                dst: ((instr & C_DST2_REG_MASK) >> C_DST2_REG_SHIFT) as usize,
                src1: ((instr & C_SRC1_REG_MASK) >> C_SRC1_REG_SHIFT) as usize,
                imm: ((instr & C_IMM_STORE_MASK1) >> C_IMM_STORE_SHIFT1) |
                     (((instr & C_IMM_STORE_MASK2) << 16) as i32 >> (C_IMM_STORE_SHIFT2 as i32 + 16)) as u32
            },
            C_STORE => Store {
                op: op,
                src1: ((instr & C_DST2_REG_MASK) >> C_DST2_REG_SHIFT) as usize,
                src2: ((instr & C_SRC1_REG_MASK) >> C_SRC1_REG_SHIFT) as usize,
                imm: ((instr & C_IMM_STORE_MASK1) >> C_IMM_STORE_SHIFT1) |
                     (((instr & C_IMM_STORE_MASK2) << 16) as i32 >> (C_IMM_STORE_SHIFT2 as i32 + 16)) as u32
            },
            C_LUI => Upper {
                op: op,
                dst: ((instr & C_DST1_REG_MASK) >> C_DST1_REG_SHIFT) as usize,
                imm: (((instr & C_IMM_MASK) << 16) as i32 >> (16 - C_IMM_UPPER_SHIFT)) as u32
            }
        })
    }
}

#[cfg(test)]
mod test {
    use std::convert::TryFrom;

    use super::Instruction;
    use super::Instruction::*;
    use super::OpCode::*;

    #[test]
    fn r_decode() {
        assert_eq!(Instruction::try_from(0x00020842).unwrap(), Register { op: ADD, dst: 1, src1: 1, src2: 2 });
    }

    #[test]
    fn r_encode() {
        let mut buf = Vec::new();
        let instr = Register { op: ADD, dst: 1, src1: 1, src2: 2 };

        instr.write_bytes(&mut buf).unwrap();
        assert_eq!(&buf[..], [0x42, 0x08, 0x02, 0x00]);
    }

    #[test]
    fn i_decode() {
        assert_eq!(Instruction::try_from(0x00010852).unwrap(), Immediate { op: ADDI, dst: 1, src1: 1, imm: 1 });
    }

    #[test]
    fn i_encode() {
        let mut buf = Vec::new();
        let instr = Immediate { op: ADDI, dst: 1, src1: 1, imm: 1 };

        instr.write_bytes(&mut buf).unwrap();
        assert_eq!(&buf[..], [0x52, 0x08, 0x01, 0x00]);
    }

    #[test]
    fn s_decode() {
        assert_eq!(Instruction::try_from(0x00020876).unwrap(), Store { op: STORE, src1: 1, src2: 2, imm: 1 });
    }

    #[test]
    fn s_encode() {
        let mut buf = Vec::new();
        let instr = Store { op: STORE, src1: 1, src2: 2, imm: 1 };

        instr.write_bytes(&mut buf).unwrap();
        assert_eq!(&buf[..], [0x76, 0x08, 0x02, 0x00]);
    }

    #[test]
    fn u_decode() {
        assert_eq!(Instruction::try_from(0x00010072).unwrap(), Upper { op: LUI, dst: 1, imm: 0x10000 });
    }

    #[test]
    fn u_encode() {
        let mut buf = Vec::new();
        let instr = Upper { op: LUI, dst: 1, imm: 0x10000 };

        instr.write_bytes(&mut buf).unwrap();
        assert_eq!(&buf[..], [0x72, 0x00, 0x01, 0x00]);
    }

    #[test]
    fn cr_decode() {
        assert_eq!(Instruction::try_from(0x1043).unwrap(), Register { op: C_ADD, dst: 1, src1: 1, src2: 2 });
    }

    #[test]
    fn cr_encode() {
        let mut buf = Vec::new();
        let instr = Register { op: C_ADD, dst: 1, src1: 1, src2: 2 };

        instr.write_bytes(&mut buf).unwrap();
        assert_eq!(&buf[..], [0x43, 0x10]);
    }

    #[test]
    fn ci_decode() {
        assert_eq!(Instruction::try_from(0x0253).unwrap(), Immediate { op: C_ADDI, dst: 1, src1: 1, imm: 1 });
    }

    #[test]
    fn ci_encode() {
        let mut buf = Vec::new();
        let instr = Immediate { op: C_ADDI, dst: 1, src1: 1, imm: 1 };

        instr.write_bytes(&mut buf).unwrap();
        assert_eq!(&buf[..], [0x53, 0x02]);
    }

    #[test]
    fn cl_decode() {
        assert_eq!(Instruction::try_from(0x1175).unwrap(), Immediate { op: C_LOAD, dst: 1, src1: 2, imm: 2 });
    }

    #[test]
    fn cl_encode() {
        let mut buf = Vec::new();
        let instr = Immediate { op: C_LOAD, dst: 1, src1: 2, imm: 2 };

        instr.write_bytes(&mut buf).unwrap();
        assert_eq!(&buf[..], [0x75, 0x11]);
    }

    #[test]
    fn cs_decode() {
        assert_eq!(Instruction::try_from(0x1177).unwrap(), Store { op: C_STORE, src1: 1, src2: 2, imm: 2 });
    }

    #[test]
    fn cs_encode() {
        let mut buf = Vec::new();
        let instr = Store { op: C_STORE, src1: 1, src2: 2, imm: 2 };

        instr.write_bytes(&mut buf).unwrap();
        assert_eq!(&buf[..], [0x77, 0x11]);
    }

    #[test]
    fn cu_decode() {
        assert_eq!(Instruction::try_from(0x0273).unwrap(), Upper { op: C_LUI, dst: 1, imm: 0x10000 });
    }

    #[test]
    fn cu_encode() {
        let mut buf = Vec::new();
        let instr = Upper { op: C_LUI, dst: 1, imm: 0x10000 };

        instr.write_bytes(&mut buf).unwrap();
        assert_eq!(&buf[..], [0x73, 0x02]);
    }

    #[test]
    fn i_imm_sign_ext() {
        assert_eq!(Instruction::try_from(0xffff0012).unwrap(), Immediate { op: ADDI, dst: 0, src1: 0, imm: 0xffffffff });
    }

    #[test]
    fn s_imm_sign_ext() {
        assert_eq!(Instruction::try_from(0xffe007f6).unwrap(), Store { op: STORE, src1: 0, src2: 0, imm: 0xffffffff });
    }

    #[test]
    fn ci_imm_sign_ext() {
        assert_eq!(Instruction::try_from(0xfe13).unwrap(), Immediate { op: C_ADDI, dst: 0, src1: 0, imm: 0xffffffff });
    }

    #[test]
    fn cl_imm_sign_ext() {
        assert_eq!(Instruction::try_from(0xe735).unwrap(), Immediate { op: C_LOAD, dst: 0, src1: 0, imm: 0xfffffffe });
    }

    #[test]
    fn cs_imm_sign_ext() {
        assert_eq!(Instruction::try_from(0xe737).unwrap(), Store { op: C_STORE, src1: 0, src2: 0, imm: 0xfffffffe });
    }

    #[test]
    fn cu_imm_sign_ext() {
        assert_eq!(Instruction::try_from(0xfe33).unwrap(), Upper { op: C_LUI, dst: 0, imm: 0xffff0000 });
    }
}
