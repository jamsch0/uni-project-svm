use std::collections::HashMap;
use std::str::FromStr;

use nom::{ErrorKind, IResult, alpha, alphanumeric, digit, not_line_ending};

use svm::{Instruction, OpCode};

#[derive(Debug, Eq, PartialEq)]
enum ImmediatePlaceholder<'a> {
    Value(u32),
    LabelAbsolute(&'a str),
    LabelRelative(&'a str)
}

#[derive(Debug, Eq, PartialEq)]
enum InstructionPlaceholder<'a> {
    Register { op: OpCode, dst: usize, src1: usize, src2: usize },
    Immediate { op: OpCode, dst: usize, src1: usize, imm: ImmediatePlaceholder<'a> },
    Store { op: OpCode, src1: usize, src2: usize, imm: ImmediatePlaceholder<'a> },
    Upper { op: OpCode, dst: usize, imm: ImmediatePlaceholder<'a> },
    StringLiteral(String)
}

impl<'a> InstructionPlaceholder<'a> {
    /// Consumes this placeholder, returning the finalised `Instruction`.
    fn into_instr(self, labels: &HashMap<&'a str, u32>, pos: u32) -> Result<Instruction, String> {
        macro_rules! replace_labels {
            ($($instr:ident { $($field:ident),+ $(@$imm:ident)* }),*) => {
                match self {
                    $(InstructionPlaceholder::$instr { $($field,)+ $($imm)* } =>
                        replace_labels!(__impl $instr { $($field),+ $(@$imm)* }),)*
                    _ => unreachable!()
                }
            };
            (__impl $instr:ident { $($field:ident),+ $(@$imm:ident)+ }) => {
                match $($imm)+ {
                    ImmediatePlaceholder::Value(imm) =>
                        Ok(Instruction::$instr { $($field,)* imm }),
                    ImmediatePlaceholder::LabelAbsolute(label) =>
                        labels.get(label)
                              .map(|&imm| Instruction::$instr { $($field,)* imm })
                              .ok_or(format!("Label not found: {}", label)),
                    ImmediatePlaceholder::LabelRelative(label) =>
                        labels.get(label)
                              .map(|&imm| Instruction::$instr { $($field,)* imm: (imm as i32 - pos as i32) as u32 })
                              .ok_or(format!("Label not found: {}", label))
                }
            };
            (__impl $instr:ident { $($field:ident),* }) => {
                Ok(Instruction::$instr { $($field),* })
            }
        }

        replace_labels!(
            Register { op, dst, src1, src2 },
            Immediate { op, dst, src1  @imm },
            Store { op, src1, src2  @imm },
            Upper { op, dst  @imm }
        )
    }

    /// Returns the size of this instruction in bytes.
    fn size(&self) -> u32 {
        use self::InstructionPlaceholder::*;

        match *self {
            Register { op, .. } | Immediate { op, .. } | Store { op, .. } | Upper { op, .. } => {
                if (op as u32) & 1 == 0 { 4 } else { 2 }
            },
            StringLiteral(ref string) => string.len() as u32
        }
    }
}

fn comment(input: &str) -> IResult<&str, ()> {
    ws!(input, map!(preceded!(char!('#'), not_line_ending), |_| ()))
}

fn identifier(input: &str) -> IResult<&str, &str> {
    // `recognize!` allows us to include the inital character in the result
    recognize!(input, preceded!(alpha, many0!(alt!(alphanumeric | tag!("_")))))
}

fn label(input: &str) -> IResult<&str, &str> {
    ws!(input, terminated!(identifier, char!(':')))
}

fn register(input: &str) -> IResult<&str, usize> {
    ws!(input, preceded!(tag_no_case!("r"), map_res!(recognize!(many1!(digit)), FromStr::from_str)))
}

#[allow(unused_variables)]
fn number_sign(input: &str) -> IResult<&str, i32> {
    // nom has some bugs / compiler ambiguities surrounding `alt!`, `opt!` etc.
    // so we have to use this workaround
    switch!(input, alt_complete!(tag!("-") | tag!("+") | tag!("")),
        "-" => value!(-1) |
        _   => value!(1)
    )
}

#[allow(unused_variables)]
fn number_radix(input: &str) -> IResult<&str, u32> {
    // nom has some bugs / compiler ambiguities surrounding `alt!`, `opt!` etc.
    // so we have to use this workaround
    switch!(input, alt_complete!(tag_no_case!("0b") | tag_no_case!("0o") | tag_no_case!("0x") | tag!("")),
        // `switch!` doesn't accept `|` in patterns, so we can't write
        // `"0b" | "0B" => ...` like we can in `match`
        "0b" => value!(2) |
        "0B" => value!(2) |
        "0o" => value!(8) |
        "0O" => value!(8) |
        "0x" => value!(16) |
        "0X" => value!(16) |
        _    => value!(10)
    )
}

fn number(input: &str) -> IResult<&str, u32> {
    do_parse!(input,
        sign: number_sign >>
        radix: number_radix >>
        value: map_res!(
            recognize!(many1!(alphanumeric)), |s| i32::from_str_radix(s, radix).map(|i| (i * sign) as u32)
        ) >>
        (value)
    )
}

fn immediate(input: &str) -> IResult<&str, ImmediatePlaceholder> {
    ws!(input, alt!(
        map!(number, |n| ImmediatePlaceholder::Value(n)) |
        map!(preceded!(char!('%'), identifier), |l| ImmediatePlaceholder::LabelAbsolute(l)) |
        map!(preceded!(char!('$'), identifier), |l| ImmediatePlaceholder::LabelRelative(l))
    ))
}

fn mnemonic(input: &str) -> IResult<&str, &str> {
    ws!(input, recognize!(
        delimited!(
            opt!(complete!(tag_no_case!("c."))),
            many1!(alpha),
            opt!(complete!(tag_no_case!(".u")))
        )
    ))
}

fn string_literal(input: &str) -> IResult<&str, InstructionPlaceholder> {
    ws!(input, delimited!(
        char!('"'),
        map!(
            recognize!(opt!(is_not!("\""))),
            |s: &str| InstructionPlaceholder::StringLiteral(s.replace("\\\\", "\\")
                                                             .replace("\\0", "\0")
                                                             .replace("\\n", "\n")
                                                             .replace("\\r", "\r")
                                                             .replace("\\t", "\t")
                                                             .replace("\\'", "\'")
                                                             .replace("\\\"", "\""))
        ),
        char!('"')
    ))
}

fn instruction_r(input: &str, op: OpCode) -> IResult<&str, InstructionPlaceholder> {
    do_parse!(input,
        dst: terminated!(register, char!(',')) >>
        src1: terminated!(register, char!(',')) >>
        src2: register >>
        (InstructionPlaceholder::Register { op, dst, src1, src2 })
    )
}

fn instruction_cr(input: &str, op: OpCode) -> IResult<&str, InstructionPlaceholder> {
    do_parse!(input,
        dst: terminated!(register, char!(',')) >>
        src2: register >>
        (InstructionPlaceholder::Register { op, dst, src1: dst, src2 })
    )
}

fn instruction_i(input: &str, op: OpCode) -> IResult<&str, InstructionPlaceholder> {
    do_parse!(input,
        dst: terminated!(register, char!(',')) >>
        src1: terminated!(register, char!(',')) >>
        imm: immediate >>
        (InstructionPlaceholder::Immediate { op, dst, src1, imm })
    )
}

fn instruction_ci(input: &str, op: OpCode) -> IResult<&str, InstructionPlaceholder> {
    do_parse!(input,
        dst: terminated!(register, char!(',')) >>
        imm: immediate >>
        (InstructionPlaceholder::Immediate { op, dst, src1: dst, imm })
    )
}

fn instruction_call(input: &str, op: OpCode) -> IResult<&str, InstructionPlaceholder> {
    do_parse!(input,
        imm: immediate >>
        (InstructionPlaceholder::Immediate { op, dst: 0, src1 : 0, imm })
    )
}

fn instruction_break(input: &str, op: OpCode) -> IResult<&str, InstructionPlaceholder> {
    value!(input, InstructionPlaceholder::Immediate { op, dst: 0, src1: 0, imm: ImmediatePlaceholder::Value(0) })
}

fn instruction_s(input: &str, op: OpCode) -> IResult<&str, InstructionPlaceholder> {
    do_parse!(input,
        src1: terminated!(register, char!(',')) >>
        src2: terminated!(register, char!(',')) >>
        imm: immediate >>
        (InstructionPlaceholder::Store { op, src1, src2, imm })
    )
}

fn instruction_u(input: &str, op: OpCode) -> IResult<&str, InstructionPlaceholder> {
    do_parse!(input,
        dst: terminated!(register, char!(',')) >>
        imm: immediate >>
        (InstructionPlaceholder::Upper { op, dst, imm })
    )
}

fn instruction(input: &str) -> IResult<&str, InstructionPlaceholder> {
    use svm::OpCode::*;

    // Can't use `switch!()` as it doesn't accept `|` in patterns,
    // so we have to manually expand it
    match map!(input, mnemonic, |s: &str| s.replace(".", "_").to_uppercase()) {
        IResult::Error(error) => IResult::Error(error),
        IResult::Incomplete(needed) => IResult::Incomplete(needed),
        IResult::Done(input, output) => match output.as_ref() {
            "BYTES" => ws!(input, string_literal),
            _ => match OpCode::from_str(&output) {
                Err(_) => IResult::Error(error_position!(ErrorKind::MapRes, input)),
                Ok(op) => match op {
                    ADD | SUB | AND | OR | XOR | SLL | SRL | SRA =>
                        instruction_r(input, op),
                    C_ADD | C_SUB | C_AND | C_OR | C_XOR | C_SLL | C_SRL | C_SRA | MV =>
                        instruction_cr(input, op),
                    ADDI | ANDI | ORI | XORI | SLLI | SRLI | SRAI | LOAD | C_LOAD =>
                        instruction_i(input, op),
                    LI | BEZ | BNZ |
                    C_ADDI | C_ANDI | C_ORI | C_XORI | C_SLLI | C_SRLI | C_SRAI | C_LI | C_BEZ | C_BNZ =>
                        instruction_ci(input, op),
                    CALL | C_CALL =>
                        instruction_call(input, op),
                    BREAK | C_BREAK =>
                        instruction_break(input, op),
                    STORE | C_STORE | BEQ | BNE | BLT | BGE | BLT_U | BGE_U =>
                        instruction_s(input, op),
                    LUI | C_LUI =>
                        instruction_u(input, op)
                }
            }
        }
    }
}

fn parse_line(input: &str) -> IResult<&str, (Option<&str>, Option<InstructionPlaceholder>)> {
    ws!(input, terminated!(
        terminated!(
            alt_complete!(
                map!(comment, |_| (None, None)) |
                map!(pair!(label, instruction), |(l, i)| (Some(l), Some(i))) |
                map!(instruction, |i| (None, Some(i))) |
                map!(label, |l| (Some(l), None))
            ),
            opt!(complete!(comment))
        ),
        eof!()
    ))
}

pub fn parse(buf: String) -> Result<Vec<u8>, String> {
    let mut labels = HashMap::new();
    let mut instrs = Vec::new();
    let mut length = 0;

    for (num, line) in buf.lines().enumerate() {
        if line.len() == 0 {
            continue;
        }

        let (label, instr) = parse_line(line).to_full_result().map_err(|_| format!("error on line {}", num + 1))?;

        if let Some(label) = label {
            labels.insert(label, length);
        }

        if let Some(instr) = instr {
            length += instr.size();
            instrs.push(instr);
        }
    }

    let mut bytes = Vec::new();
    let mut length = 0;

    for instr in instrs {
        length += instr.size();

        match instr {
            InstructionPlaceholder::StringLiteral(string) => bytes.extend(string.bytes()),
            _ => instr.into_instr(&labels, length)?.write_bytes(&mut bytes).map_err(|e| format!("{:?}", e))?
        }
    }

    Ok(bytes)
}

#[cfg(test)]
mod test {
    use nom::IResult::Done;

    use svm::OpCode::*;

    use super::{ImmediatePlaceholder, InstructionPlaceholder};

    #[test]
    fn comment() {
        assert_eq!(super::comment("# This is a comment..."), Done("", ()));
    }

    #[test]
    fn identifier() {
        assert_eq!(super::identifier("Label_1"), Done("", "Label_1"));
    }

    #[test]
    fn label() {
        assert_eq!(super::label("Label_1:"), Done("", "Label_1"));
    }

    #[test]
    fn register() {
        assert_eq!(super::register("r31"), Done("", 31));
    }

    #[test]
    fn number_sign() {
        assert_eq!(super::number_sign(""), Done("", 1));
        assert_eq!(super::number_sign("+"), Done("", 1));
        assert_eq!(super::number_sign("-"), Done("", -1));
    }

    #[test]
    fn number_radix() {
        assert_eq!(super::number_radix(""), Done("", 10));
        assert_eq!(super::number_radix("0b"), Done("", 2));
        assert_eq!(super::number_radix("0o"), Done("", 8));
        assert_eq!(super::number_radix("0x"), Done("", 16));
    }

    #[test]
    fn number() {
        assert_eq!(super::number("10"), Done("", 10));
        assert_eq!(super::number("0x1f"), Done("", 0x1f));
        assert_eq!(super::number("-4"), Done("", -4i32 as u32));
        assert_eq!(super::number("-0b101"), Done("", -0b101i32 as u32));
    }

    #[test]
    fn immediate() {
        assert_eq!(super::immediate("-2"), Done("", ImmediatePlaceholder::Value(-2i32 as u32)));
        assert_eq!(super::immediate("%label1"), Done("", ImmediatePlaceholder::LabelAbsolute("label1")));
        assert_eq!(super::immediate("$label1"), Done("", ImmediatePlaceholder::LabelRelative("label1")));
    }

    #[test]
    fn mnemonic() {
        assert_eq!(super::mnemonic("addi"), Done("", "addi"));
        assert_eq!(super::mnemonic("c.addi"), Done("", "c.addi"));
    }

    #[test]
    fn instruction_r() {
        assert_eq!(super::instruction_r("r0, r0, r1", ADD),
            Done("", InstructionPlaceholder::Register { op: ADD, dst: 0, src1: 0, src2: 1 }));
    }

    #[test]
    fn instruction_i() {
        assert_eq!(super::instruction_i("r0, r0, $label", ADDI),
            Done("", InstructionPlaceholder::Immediate {
                op: ADDI, dst: 0, src1: 0, imm: ImmediatePlaceholder::LabelRelative("label")}));
    }

    #[test]
    fn instruction_s() {
        assert_eq!(super::instruction_s("r0, r1, -4", STORE),
            Done("", InstructionPlaceholder::Store {
                op: STORE, src1: 0, src2: 1, imm: ImmediatePlaceholder::Value(-4i32 as u32) }));
    }

    #[test]
    fn instruction_u() {
        assert_eq!(super::instruction_u("r1, 0x1fa800c5", LUI),
            Done("", InstructionPlaceholder::Upper { op: LUI, dst: 1, imm: ImmediatePlaceholder::Value(0x1fa800c5) }));
    }

    #[test]
    fn instruction_cr() {
        assert_eq!(super::instruction_cr("r3, r4", C_ADD),
            Done("", InstructionPlaceholder::Register { op: C_ADD, dst: 3, src1: 3, src2: 4 }));
    }

    #[test]
    fn instruction_ci() {
        assert_eq!(super::instruction_ci("r0, %label", C_ADDI),
            Done("", InstructionPlaceholder::Immediate {
                op: C_ADDI, dst: 0, src1: 0, imm: ImmediatePlaceholder::LabelAbsolute("label") }));
    }

    #[test]
    fn instruction_cl() {
        assert_eq!(super::instruction_i("r3, r1, 0", C_LOAD),
            Done("", InstructionPlaceholder::Immediate {
                op: C_LOAD, dst: 3, src1: 1, imm: ImmediatePlaceholder::Value(0) }));
    }

    #[test]
    fn instruction_cs() {
        assert_eq!(super::instruction_s("r1, r3, -4", C_STORE),
            Done("", InstructionPlaceholder::Store {
                op: C_STORE, src1: 1, src2: 3, imm: ImmediatePlaceholder::Value(-4i32 as u32) }));
    }

    #[test]
    fn instruction_cu() {
        assert_eq!(super::instruction_u("r2, %label", C_LUI),
            Done("", InstructionPlaceholder::Upper {
                op: C_LUI, dst: 2, imm: ImmediatePlaceholder::LabelAbsolute("label") }));
    }

    #[test]
    fn instruction() {
        assert_eq!(super::instruction("add r0, r0, r1"),
            Done("", InstructionPlaceholder::Register { op: ADD, dst: 0, src1: 0, src2: 1 }));
        
        assert_eq!(super::instruction("addi r0, r0, 0xff"),
            Done("", InstructionPlaceholder::Immediate {
                op: ADDI, dst: 0, src1: 0, imm: ImmediatePlaceholder::Value(0xff) }));

        assert_eq!(super::instruction("store r0, r1, -4"),
            Done("", InstructionPlaceholder::Store {
                op: STORE, src1: 0, src2: 1, imm: ImmediatePlaceholder::Value(-4i32 as u32) }));
        
        assert_eq!(super::instruction("lui r1, %label"),
            Done("", InstructionPlaceholder::Upper {
                op: LUI, dst: 1, imm: ImmediatePlaceholder::LabelAbsolute("label") }));
        
        assert_eq!(super::instruction("c.add r0, r1"),
            Done("", InstructionPlaceholder::Register { op: C_ADD, dst: 0, src1: 0, src2: 1 }));
        
        assert_eq!(super::instruction("c.addi r0, 0xff"),
            Done("", InstructionPlaceholder::Immediate {
                op: C_ADDI, dst: 0, src1: 0, imm: ImmediatePlaceholder::Value(0xff) }));
        
        assert_eq!(super::instruction("c.load r3, r1, 0"),
            Done("", InstructionPlaceholder::Immediate {
            op: C_LOAD, dst: 3, src1: 1, imm: ImmediatePlaceholder::Value(0) }));
        
        assert_eq!(super::instruction("c.store r1, r3, -4"),
            Done("", InstructionPlaceholder::Store {
                op: C_STORE, src1: 1, src2: 3, imm: ImmediatePlaceholder::Value(-4i32 as u32) }));
        
        assert_eq!(super::instruction("c.lui r2, $label"),
            Done("", InstructionPlaceholder::Upper {
                op: C_LUI, dst: 2, imm: ImmediatePlaceholder::LabelRelative("label") }));
    }

    #[test]
    fn parse_line() {
        assert_eq!(super::parse_line(" # add r0, r0, r1 "), Done("", (None, None)));

        assert_eq!(super::parse_line(" add r0, r0, r1   # ignore me"),
            Done("", (None, Some(InstructionPlaceholder::Register { op: ADD, dst: 0, src1: 0, src2: 1 }))));

        assert_eq!(super::parse_line("label:"), Done("", (Some("label"), None)));

        assert_eq!(super::parse_line(" label :\tadd r0,r0,r1 "),
            Done("", (Some("label"), Some(InstructionPlaceholder::Register { op: ADD, dst: 0, src1: 0, src2: 1 }))));
    }

    #[test]
    fn parse() {
        assert_eq!(super::parse("add r0, r0, r1".to_owned()), Ok(vec![0x02, 0x00, 0x01, 0x00]));
        assert_eq!(super::parse("addi r0, r0, 4".to_owned()), Ok(vec![0x12, 0x00, 0x04, 0x00]));

        assert_eq!(super::parse("label:\n add r2, r2, r3\n addi r0, r0, $label".to_owned()),
            Ok(vec![0x82, 0x10, 0x03, 0x00, 0x12, 0x00, 0xf8, 0xff]));
        
        assert_eq!(super::parse("load r0, r2, %label\n label:".to_owned()),
            Ok(vec![0x34, 0x10, 0x04, 0x00]));
    }
}
