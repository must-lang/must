use std::collections::HashMap;

use crate::bytecode::ir;

pub fn run(prog: ir::Prog) -> Value {
    let mut vm = VM {
        funcs: &prog.funcs,
        stack: [Value::Null; 1024],
        stack_ptr: 0,
    };

    vm.call_func_name("main", vec![])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    Null,
    True,
    False,
    Int(usize),
    Ptr(usize),
}

struct VM<'a> {
    funcs: &'a HashMap<String, ir::Func>,
    stack: [Value; 1024],
    stack_ptr: usize,
}

impl<'a> VM<'a> {
    pub fn call_func_name(&mut self, name: &str, mut args: Vec<Value>) -> Value {
        match name {
            "printnum" => {
                let x = args.pop().unwrap();
                if let Value::Int(n) = x {
                    println!("output: {}", n)
                } else {
                    panic!()
                }
                Value::Null
            }
            _ => {
                let func = self.funcs.get(name).unwrap();
                self.call_func(func, args)
            }
        }
    }

    pub fn call_func(&mut self, func: &ir::Func, args: Vec<Value>) -> Value {
        let mut next_block_id = 0;
        let mut regs: Vec<Value> = vec![Value::Null; func.register_count];

        regs[..args.len()].copy_from_slice(&args);

        // base ptr
        let bp = self.stack_ptr;

        let mut slot_offsets = vec![0; func.stack_slots.len()];
        let mut total_frame_size = 0;

        for (i, slot) in func.stack_slots.iter().enumerate() {
            slot_offsets[i] = total_frame_size;
            total_frame_size += slot.size;
        }

        self.stack_ptr += total_frame_size;

        let v = loop {
            for instr in &func.blocks[next_block_id].instrs {
                match instr {
                    ir::Inst::LoadInt(reg, n) => regs[reg.0] = Value::Int(*n),
                    ir::Inst::Add(reg, reg1, reg2) => match (regs[reg1.0], regs[reg2.0]) {
                        (Value::Int(a), Value::Int(b)) => regs[reg.0] = Value::Int(a + b),
                        (Value::Ptr(a), Value::Int(b)) => regs[reg.0] = Value::Ptr(a + b),
                        (Value::Int(a), Value::Ptr(b)) => regs[reg.0] = Value::Ptr(a + b),
                        _ => panic!("{:?}", (regs[reg1.0], regs[reg2.0])),
                    },
                    ir::Inst::Assign(reg, reg1) => regs[reg.0] = regs[reg1.0],
                    ir::Inst::FnCall(reg, name, args) => {
                        let args = args.iter().map(|id| regs[id.0]).collect();
                        regs[reg.0] = self.call_func_name(name, args);
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
                        let addr = bp + slot_offsets[ss.0] + offset;
                        regs[reg.0] = self.stack[addr]
                    }
                    ir::Inst::StackStore(ss, offset, reg) => {
                        let addr = bp + slot_offsets[ss.0] + offset;
                        self.stack[addr] = regs[reg.0]
                    }
                    ir::Inst::StackAddr(reg, ss, offset) => {
                        let addr = bp + slot_offsets[ss.0] + offset;
                        regs[reg.0] = Value::Ptr(addr);
                    }
                    ir::Inst::Load(reg, reg_ptr, offset) => {
                        if let Value::Ptr(addr) = regs[reg_ptr.0] {
                            regs[reg.0] = self.stack[addr + offset];
                        } else {
                            panic!("Load expected a Ptr value in register");
                        }
                    }
                    ir::Inst::Store(reg_ptr, offset, reg_val) => {
                        if let Value::Ptr(addr) = regs[reg_ptr.0] {
                            self.stack[addr + offset] = regs[reg_val.0];
                        } else {
                            panic!("Store expected a Ptr value in destination register");
                        }
                    }
                    ir::Inst::MemCopy { src, dst, len } => match (regs[src.0], regs[dst.0]) {
                        (Value::Ptr(src), Value::Ptr(dst)) => {
                            for i in 0_usize..*len {
                                self.stack[dst + i] = self.stack[src + i]
                            }
                        }
                        _ => panic!(),
                    },
                    ir::Inst::AddImm(reg, reg1, n) => match regs[reg1.0] {
                        Value::Int(m) => regs[reg.0] = Value::Int(n + m),
                        Value::Ptr(m) => regs[reg.0] = Value::Ptr(n + m),
                        _ => panic!("{:?}", regs[reg1.0]),
                    },
                    ir::Inst::CmpLe(reg, reg1, reg2) => {
                        regs[reg.0] = if regs[reg1.0] <= regs[reg2.0] {
                            Value::True
                        } else {
                            Value::False
                        }
                    }
                    ir::Inst::Mul(reg, reg1, reg2) => match (regs[reg1.0], regs[reg2.0]) {
                        (Value::Int(a), Value::Int(b)) => regs[reg.0] = Value::Int(a * b),
                        _ => panic!(),
                    },
                    ir::Inst::Sub(reg, reg1, reg2) => match (regs[reg1.0], regs[reg2.0]) {
                        (Value::Int(a), Value::Int(b)) => regs[reg.0] = Value::Int(a - b),
                        _ => panic!(),
                    },
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
        self.stack_ptr = bp;
        v
    }
}
