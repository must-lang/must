use crate::bytecode::ir;
use crate::bytecode::place::Place;
use std::collections::HashMap;
use std::fmt;

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
    Sub(Reg, Reg, Reg),            // r1 <- r2 + r3
    Mul(Reg, Reg, Reg),            // r1 <- r2 * r3
    AddImm(Reg, Reg, usize),       // r1 <- r2 + r3
    Assign(Reg, Reg),              // r1 <- r2
    FnCall(Reg, String, Vec<Reg>), // r1 <- "name"(regs)
    CmpEq(Reg, Reg, Reg),          // r1 <- r2 == r3
    CmpLe(Reg, Reg, Reg),          // r1 <- r2 <= r3

    StackAddr(Reg, StackSlotId, usize),  // r1 <- ss + offset
    StackLoad(Reg, StackSlotId, usize),  // r1 <- *(ss + offset)
    StackStore(StackSlotId, usize, Reg), // ss + offset <- r1

    Load(Reg, Reg, usize),  // r1 <- *(r2 + offset)
    Store(Reg, usize, Reg), // *(r1 + offset) <- r2

    MemCopy { src: Reg, dst: Reg, len: usize },
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

impl ir::IrBuilder {
    pub fn load_from_place(&mut self, place: Place) -> ir::Reg {
        match place {
            Place::DynamicPtr { base, offset } => {
                let reg = self.new_reg();
                self.push_instr(ir::Inst::Load(reg, base, offset));
                reg
            }
            Place::Stack { slot, offset } => {
                let reg = self.new_reg();
                self.push_instr(ir::Inst::StackLoad(reg, slot, offset));
                reg
            }
            Place::Reg(reg) => reg,
        }
    }

    pub fn store_to_place(&mut self, dest: Place, reg: ir::Reg) {
        match dest {
            Place::DynamicPtr { base, offset } => {
                self.push_instr(ir::Inst::Store(base, offset, reg))
            }
            Place::Stack { slot, offset } => {
                self.push_instr(ir::Inst::StackStore(slot, offset, reg))
            }
            Place::Reg(reg_dest) => self.push_instr(ir::Inst::Assign(reg_dest, reg)),
        }
    }
}

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
            // --- Constants & Moves ---
            Inst::LoadInt(dst, val) => write!(f, "r{} = int {}", dst.0, val),
            Inst::LoadBool(dst, val) => write!(f, "r{} = bool {}", dst.0, val),
            Inst::Assign(dst, src) => write!(f, "r{} = mov r{}", dst.0, src.0),

            // --- Math & Logic ---
            Inst::Add(dst, lhs, rhs) => write!(f, "r{} = add r{}, r{}", dst.0, lhs.0, rhs.0),
            Inst::Sub(dst, lhs, rhs) => write!(f, "r{} = sub r{}, r{}", dst.0, lhs.0, rhs.0),
            Inst::Mul(dst, lhs, rhs) => write!(f, "r{} = mul r{}, r{}", dst.0, lhs.0, rhs.0),
            Inst::AddImm(dst, lhs, imm) => write!(f, "r{} = add_imm r{}, {}", dst.0, lhs.0, imm),
            Inst::CmpEq(dst, lhs, rhs) => write!(f, "r{} = eq r{}, r{}", dst.0, lhs.0, rhs.0),
            Inst::CmpLe(dst, lhs, rhs) => write!(f, "r{} = le r{}, r{}", dst.0, lhs.0, rhs.0),

            // --- Function Calls ---
            Inst::FnCall(dst, name, args) => {
                let args_str = args
                    .iter()
                    .map(|r| format!("r{}", r.0))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "r{} = call {}({})", dst.0, name, args_str)
            }

            // --- Stack Memory (sX) ---
            Inst::StackAddr(dst, ss, offset) => {
                write!(f, "r{} = &s{} + {}", dst.0, ss.0, offset)
            }
            Inst::StackLoad(dst, ss, offset) => {
                write!(f, "r{} = [s{} + {}]", dst.0, ss.0, offset)
            }
            Inst::StackStore(ss, offset, src) => {
                write!(f, "[s{} + {}] = r{}", ss.0, offset, src.0)
            }

            // --- Dynamic Pointers (rX) ---
            Inst::Load(dst, ptr, offset) => {
                write!(f, "r{} = [r{} + {}]", dst.0, ptr.0, offset)
            }
            Inst::Store(ptr, offset, src) => {
                write!(f, "[r{} + {}] = r{}", ptr.0, offset, src.0)
            }
            Inst::MemCopy { src, dst, len } => {
                write!(f, "memcpy dst=r{}, src=r{}, len={}", dst.0, src.0, len)
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
