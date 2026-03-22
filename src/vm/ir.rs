use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Prog {
    pub funcs: HashMap<String, Func>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Reg(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub struct Func {
    pub register_count: usize,
    pub blocks: Vec<Block>,
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
    pub next_reg: usize,
}

impl IrBuilder {
    pub fn new() -> Self {
        Self {
            blocks: vec![Block {
                instrs: vec![],
                terminator: Terminator::Return(Reg(0)),
            }],
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
}
