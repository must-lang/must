use std::collections::HashMap;

pub mod ir;
pub mod lower;

pub fn run(prog: ir::Prog) -> Value {
    let mut vm = VM {
        funcs: &prog.funcs,
        stack: vec![],
    };

    vm.call_func("main", vec![])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Null,
    True,
    False,
    Int(usize),
}

struct VM<'a> {
    funcs: &'a HashMap<String, ir::Func>,
    stack: Vec<Value>,
}

impl<'a> VM<'a> {
    pub fn call_func(&mut self, name: &str, args: Vec<Value>) -> Value {
        let func = self.funcs.get(name).unwrap();
        let mut next_block_id = 0;
        let mut regs: Vec<Value> = vec![Value::Null; func.register_count];
        for i in 0..args.len() {
            regs[i] = args[i]
        }
        // base ptr
        let bp = self.stack.len();

        let v = loop {
            for instr in &func.blocks[next_block_id].instrs {
                match instr {
                    ir::Inst::LoadInt(reg, n) => regs[reg.0] = Value::Int(*n),
                    ir::Inst::Add(reg, reg1, reg2) => match (regs[reg1.0], regs[reg2.0]) {
                        (Value::Int(a), Value::Int(b)) => regs[reg.0] = Value::Int(a + b),
                        _ => panic!(),
                    },
                    ir::Inst::Assign(reg, reg1) => regs[reg.0] = regs[reg1.0],
                    ir::Inst::FnCall(reg, name, args) => {
                        let args = args.iter().map(|id| regs[id.0]).collect();
                        regs[reg.0] = self.call_func(name, args);
                    }
                    ir::Inst::LoadBool(reg, true) => regs[reg.0] = Value::True,
                    ir::Inst::LoadBool(reg, false) => regs[reg.0] = Value::False,
                    ir::Inst::CmpEq(reg, reg1, reg2) => {
                        regs[reg.0] = if regs[reg1.0] == regs[reg2.0] {
                            Value::True
                        } else {
                            Value::False
                        }
                    }
                    ir::Inst::StackLoad(reg, ss, offset) => {
                        regs[reg.0] = self.stack[bp + ss.0 + offset]
                    }
                    ir::Inst::StackStore(ss, offset, reg) => {
                        self.stack[bp + ss.0 + offset] = regs[reg.0]
                    }
                }
            }

            match func.blocks[next_block_id].terminator {
                ir::Terminator::Return(reg) => break regs[reg.0],
                ir::Terminator::Jump(block_id) => next_block_id = block_id.0,
                ir::Terminator::BranchIf { cond, th, el } => {
                    next_block_id = if regs[cond.0] == Value::True { th } else { el }.0
                }
            }
        };

        // free the stack
        self.stack.truncate(bp);
        v
    }
}
