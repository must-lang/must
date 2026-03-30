use crate::bytecode::ir;

#[derive(Debug, Clone, Copy)]
pub enum Place {
    Reg(ir::Reg),
    DynamicPtr {
        base: ir::Reg,
        offset: usize,
    },
    Stack {
        slot: ir::StackSlotId,
        offset: usize,
    },
}

impl Place {
    pub fn add_offset(self, n: usize) -> Self {
        match self {
            Place::DynamicPtr { base, offset } => Place::DynamicPtr {
                base,
                offset: offset + n,
            },
            Place::Stack { slot, offset } => Place::Stack {
                slot,
                offset: offset + n,
            },
            Place::Reg(_) => panic!(),
        }
    }

    pub fn as_addr(&self, builder: &mut ir::IrBuilder) -> ir::Reg {
        match self {
            Place::Reg(_) => panic!(),
            Place::DynamicPtr { base, offset } => {
                let reg = builder.new_reg();
                builder.push_instr(ir::Inst::AddImm(reg, *base, *offset));
                reg
            }
            Place::Stack { slot, offset } => {
                let reg = builder.new_reg();
                builder.push_instr(ir::Inst::StackAddr(reg, *slot, *offset));
                reg
            }
        }
    }
}
