use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Prog {
    pub funcs: HashMap<String, Func>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Reg(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StackSlotId(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub struct Func {
    pub register_count: usize,
    pub blocks: Vec<Block>,
    pub stack_slots: Vec<StackSlot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StackSlot {
    pub size: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub instrs: Vec<Inst>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inst {
    LoadInt(Reg, usize),           // r1 <- int
    LoadBool(Reg, bool),           // r1 <- bool
    Add(Reg, Reg, Reg),            // r1 <- r2 + r3
    Assign(Reg, Reg),              // r1 <- r2
    FnCall(Reg, String, Vec<Reg>), // r1 <- "name"(regs)
    CmpEq(Reg, Reg, Reg),          // r1 <- r2 == r3

    StackLoad(Reg, StackSlotId, usize),  // r1 <- *(ss + offset)
    StackStore(StackSlotId, usize, Reg), // ss + offset <- r1
}

#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    Return(Reg),
    Jump(BlockId),
    BranchIf { cond: Reg, th: BlockId, el: BlockId },
}

pub struct IrBuilder {
    pub blocks: Vec<Block>,
    pub current_block: BlockId,
    pub stack_slots: Vec<StackSlot>,
    pub next_reg: usize,
}

impl IrBuilder {
    pub fn new() -> Self {
        Self {
            blocks: vec![Block {
                instrs: vec![],
                terminator: Terminator::Return(Reg(0)),
            }],
            stack_slots: vec![],
            current_block: BlockId(0),
            next_reg: 0,
        }
    }

    pub fn new_reg(&mut self) -> Reg {
        let r = Reg(self.next_reg);
        self.next_reg += 1;
        r
    }

    pub fn push_instr(&mut self, instr: Inst) {
        self.blocks[self.current_block.0].instrs.push(instr);
    }

    pub fn finish_block(&mut self, t: Terminator) {
        self.blocks[self.current_block.0].terminator = t
    }

    pub fn new_block(&mut self) -> BlockId {
        self.blocks.push(Block {
            instrs: vec![],
            terminator: Terminator::Return(Reg(0)),
        });
        BlockId(self.blocks.len() - 1)
    }

    pub fn switch_to_block(&mut self, id: BlockId) {
        self.current_block = id
    }

    pub fn new_stack_slot(&mut self, size: usize) -> StackSlotId {
        self.stack_slots.push(StackSlot { size });
        StackSlotId(self.stack_slots.len() - 1)
    }
}

use std::fmt;

impl fmt::Display for Prog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (name, func) in &self.funcs {
            writeln!(f, "fn {}({}):", name, func.register_count)?;
            for (id, block) in func.blocks.iter().enumerate() {
                writeln!(f, "  block {}:", id)?;
                for inst in &block.instrs {
                    writeln!(f, "    {}", inst)?;
                }
                writeln!(f, "    {}", block.terminator)?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

impl fmt::Display for Inst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Inst::LoadInt(r, v) => write!(f, "r{} = load_int {}", r.0, v),
            Inst::LoadBool(r, v) => write!(f, "r{} = load_bool {}", r.0, v),
            Inst::Add(dst, lhs, rhs) => write!(f, "r{} = add r{} r{}", dst.0, lhs.0, rhs.0),
            Inst::Assign(dst, src) => write!(f, "r{} = assign r{}", dst.0, src.0),
            Inst::CmpEq(dst, lhs, rhs) => write!(f, "r{} = eq r{} r{}", dst.0, lhs.0, rhs.0),
            Inst::FnCall(dst, name, args) => {
                let args_str: Vec<String> = args.iter().map(|r| format!("r{}", r.0)).collect();
                write!(f, "r{} = call {} [{}]", dst.0, name, args_str.join(" "))
            }
            Inst::StackLoad(reg, ss, offset) => {
                write!(f, "r{} = load s{} + {}", reg.0, ss.0, offset)
            }
            Inst::StackStore(ss, offset, reg) => {
                write!(f, "s{} + {} = r{}", ss.0, offset, reg.0)
            }
        }
    }
}

impl fmt::Display for Terminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Terminator::Return(r) => write!(f, "ret r{}", r.0),
            Terminator::Jump(b) => write!(f, "jmp b{}", b.0),
            Terminator::BranchIf { cond, th, el } => {
                write!(f, "br r{} b{} b{}", cond.0, th.0, el.0)
            }
        }
    }
}
