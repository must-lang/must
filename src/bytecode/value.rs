use crate::bytecode::{ir, place};

#[derive(Debug, Clone, Copy)]
pub enum Value {
    Unit,
    Int(usize),
    LVal(place::Place),
}

impl Value {
    pub fn load_scalar(self, builder: &mut ir::IrBuilder) -> ir::Reg {
        match self {
            Value::LVal(place) => builder.load_from_place(place),
            Value::Unit => panic!(),
            Value::Int(n) => {
                let reg = builder.new_reg();
                builder.push_instr(ir::Inst::LoadInt(reg, n));
                reg
            }
        }
    }

    pub fn write_to(self, dest: place::Place, size: usize, builder: &mut ir::IrBuilder) {
        match self {
            Value::LVal(src) => match (src, dest) {
                (place::Place::Reg(reg), _) => builder.store_to_place(dest, reg),
                (_, place::Place::Reg(_)) => {
                    let reg = builder.load_from_place(src);
                    builder.store_to_place(dest, reg)
                }
                (_, _) => {
                    let r1 = src.as_addr(builder);
                    let r2 = dest.as_addr(builder);
                    builder.push_instr(ir::Inst::MemCopy {
                        src: r1,
                        dst: r2,
                        len: size,
                    });
                }
            },
            Value::Unit => (),
            Value::Int(_) => {
                let reg = self.load_scalar(builder);
                builder.store_to_place(dest, reg);
            }
        }
    }
}
