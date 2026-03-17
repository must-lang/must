use std::{collections::HashMap, sync::Arc};

use crate::ir;

#[derive(Debug, Clone)]
pub enum Value {
    Tuple(Vec<Value>),
    True,
    False,
    Num(usize),
    Ptr(usize),
    Func(Vec<ir::Pattern>, Arc<ir::Expr>),
}

#[derive(Debug)]
pub struct Memory {
    stack: Vec<Value>,
}

impl Memory {
    pub fn new() -> Self {
        Self { stack: vec![] }
    }

    pub fn alloc(&mut self, v: Value) -> usize {
        self.stack.push(v);
        self.stack.len() - 1
    }

    pub fn store(&mut self, ptr: usize, v: Value) {
        self.stack[ptr] = v
    }

    pub fn load(&mut self, ptr: usize) -> &Value {
        &self.stack[ptr]
    }

    pub fn get_sp(&self) -> usize {
        self.stack.len()
    }

    pub fn set_sp(&mut self, addr: usize) {
        self.stack.truncate(addr);
    }
}

#[derive(Debug, Clone)]
pub enum Env {
    Empty,
    Extend(Arc<Env>, HashMap<String, usize>),
}

impl Env {
    fn lookup(&self, x: &str) -> Option<usize> {
        match self {
            Env::Empty => None,
            Env::Extend(env, hash_map) => hash_map.get(x).copied().or_else(|| env.lookup(x)),
        }
    }

    fn extend<T: Into<HashMap<std::string::String, usize>>>(self: Arc<Self>, vals: T) -> Arc<Self> {
        let map = vals.into();
        Arc::new(Self::Extend(self, map))
    }
}

pub fn eval_expr(env: Arc<Env>, mem: &mut Memory, e: &ir::Expr) -> Value {
    match e {
        ir::Expr::Num(n) => Value::Num(*n),
        ir::Expr::Var(x) => {
            let addr = env.lookup(x).expect("runtime error");
            mem.load(addr).clone()
        }
        ir::Expr::FnCall(name, exprs) => {
            let v_fun = env.lookup(name).expect("runtime error");
            let v_fun = mem.load(v_fun).clone();
            let (args, ir) = match v_fun {
                Value::Func(items, expr) => (items, expr),
                _ => panic!("runtime error"),
            };
            assert_eq!(args.len(), exprs.len());
            let sp = mem.get_sp();

            let vals: Vec<Value> = exprs
                .iter()
                .map(|e| eval_expr(env.clone(), mem, e))
                .collect();

            let mut bindings = vec![];

            for (pat, val) in args.into_iter().zip(vals) {
                if let Some(binds) = try_match(&pat, &val) {
                    bindings.extend(binds)
                } else {
                    panic!("runtime error")
                }
            }

            let bindings = bindings.into_iter().map(|(s, v)| (s, mem.alloc(v)));

            let env = env.extend(bindings.into_iter().collect::<HashMap<_, _>>());

            let res = eval_expr(env, mem, &ir);

            mem.set_sp(sp);
            res
        }
        ir::Expr::Let(pat, e1, e2) => {
            let val = eval_expr(env.clone(), mem, e1);
            if let Some(bindings) = try_match(pat, &val) {
                let env = env.extend(
                    bindings
                        .into_iter()
                        .map(|(s, v)| (s, mem.alloc(v)))
                        .collect::<HashMap<_, _>>(),
                );
                eval_expr(env, mem, e2)
            } else {
                panic!("runtime error")
            }
        }
        ir::Expr::Builtin(name, exprs) => {
            let mut vals: Vec<Value> = exprs
                .iter()
                .map(|e| eval_expr(env.clone(), mem, e))
                .rev()
                .collect();
            match name.as_str() {
                "intadd" => {
                    let x = vals.pop();
                    let y = vals.pop();
                    let res = match (x, y) {
                        (Some(Value::Num(x)), Some(Value::Num(y))) => x + y,
                        _ => panic!("runtime error"),
                    };
                    Value::Num(res)
                }
                "intsub" => {
                    let x = vals.pop();
                    let y = vals.pop();
                    let res = match (x, y) {
                        (Some(Value::Num(x)), Some(Value::Num(y))) => x - y,
                        _ => panic!("runtime error"),
                    };
                    Value::Num(res)
                }
                "intle" => {
                    let x = vals.pop();
                    let y = vals.pop();
                    match (x, y) {
                        (Some(Value::Num(x)), Some(Value::Num(y))) => {
                            if x <= y {
                                Value::True
                            } else {
                                Value::False
                            }
                        }
                        _ => panic!("runtime error"),
                    }
                }
                "printnum" => {
                    let x = vals.pop();
                    match x {
                        Some(Value::Num(x)) => println!("output: {x}"),
                        _ => panic!("runtime error"),
                    }
                    Value::Tuple(vec![])
                }
                _ => panic!("runtime error: unknown builtin function"),
            }
        }
        ir::Expr::True => Value::True,
        ir::Expr::False => Value::False,
        ir::Expr::Match(expr, cls) => {
            let val = eval_expr(env.clone(), mem, expr);
            for (pat, expr) in cls {
                if let Some(bindings) = try_match(pat, &val) {
                    let env = env.extend(
                        bindings
                            .into_iter()
                            .map(|(s, v)| (s, mem.alloc(v)))
                            .collect::<HashMap<_, _>>(),
                    );
                    return eval_expr(env, mem, expr);
                }
            }
            panic!("non-exhaustive pattern-matching")
        }
        ir::Expr::Tuple(exprs) => Value::Tuple(
            exprs
                .iter()
                .map(|e| eval_expr(env.clone(), mem, e))
                .collect(),
        ),
        ir::Expr::Assign(e1, e2) => {
            let addr = eval_place(env.clone(), mem, e1);
            let v = eval_expr(env, mem, e2);
            mem.store(addr, v);
            Value::Tuple(vec![])
        }
        ir::Expr::AddressOf(expr) => Value::Ptr(eval_place(env, mem, expr)),
        ir::Expr::Load(expr) => {
            let v = eval_expr(env, mem, expr);
            match v {
                Value::Ptr(ptr) => mem.load(ptr).clone(),
                _ => panic!("rt error"),
            }
        }
        ir::Expr::Store(e1, e2) => {
            let ptr = match eval_expr(env.clone(), mem, e1) {
                Value::Ptr(ptr) => ptr,
                _ => panic!("rt error"),
            };
            let v = eval_expr(env, mem, e2);
            mem.store(ptr, v);
            Value::Tuple(vec![])
        }
        ir::Expr::Array(exprs) => {
            let ptr = mem.get_sp();
            for e in exprs {
                let val = eval_expr(env.clone(), mem, e);
                mem.alloc(val);
            }
            Value::Ptr(ptr)
        }
        ir::Expr::Index(expr, expr1) => {
            let id = match eval_expr(env.clone(), mem, expr1) {
                Value::Num(id) => id,
                _ => panic!("rt error"),
            };
            let ptr = match eval_expr(env, mem, expr) {
                Value::Ptr(ptr) => ptr,
                _ => panic!("rt error"),
            };
            mem.load(ptr + id).clone()
        }
    }
}

pub fn eval_place(env: Arc<Env>, mem: &mut Memory, e: &ir::Expr) -> usize {
    match e {
        ir::Expr::Var(x) => env.lookup(x).unwrap(),
        ir::Expr::Let(pat, e1, e2) => {
            let val = eval_expr(env.clone(), mem, e1);
            if let Some(bindings) = try_match(pat, &val) {
                let env = env.extend(
                    bindings
                        .into_iter()
                        .map(|(s, v)| (s, mem.alloc(v)))
                        .collect::<HashMap<_, _>>(),
                );
                eval_place(env, mem, e2)
            } else {
                panic!("runtime error")
            }
        }
        ir::Expr::Match(expr, cls) => {
            let val = eval_expr(env.clone(), mem, expr);
            for (pat, expr) in cls {
                if let Some(bindings) = try_match(pat, &val) {
                    let env = env.extend(
                        bindings
                            .into_iter()
                            .map(|(s, v)| (s, mem.alloc(v)))
                            .collect::<HashMap<_, _>>(),
                    );
                    return eval_place(env, mem, expr);
                }
            }
            panic!("non-exhaustive pattern-matching")
        }
        ir::Expr::Load(expr) => {
            let ptr = eval_place(env, mem, expr);
            let v = mem.load(ptr);
            match v {
                Value::Ptr(ptr) => *ptr,
                _ => panic!("rt error"),
            }
        }
        ir::Expr::Index(arr, id) => {
            let id = match eval_expr(env.clone(), mem, id) {
                Value::Num(id) => id,
                _ => panic!("rt error"),
            };
            let ptr = match eval_expr(env, mem, arr) {
                Value::Ptr(ptr) => ptr,
                _ => panic!("rt error"),
            };
            ptr + id
        }
        _ => panic!("rt error"),
    }
}

fn try_match(pat: &ir::Pattern, val: &Value) -> Option<Vec<(String, Value)>> {
    match (pat, val) {
        (ir::Pattern::Wildcard, _) => Some(vec![]),
        (ir::Pattern::True, Value::True) => Some(vec![]),
        (ir::Pattern::False, Value::False) => Some(vec![]),

        (ir::Pattern::Tuple(pats), Value::Tuple(vals)) => {
            assert_eq!(pats.len(), vals.len());
            let mut binds = vec![];
            for (pat, val) in pats.iter().zip(vals) {
                binds.extend(try_match(pat, val)?);
            }
            Some(binds)
        }

        (ir::Pattern::Var(name), val) => Some(vec![(name.clone(), val.clone())]),

        _ => None,
    }
}

pub fn eval(prog: ir::Program) -> Value {
    let env = Arc::new(Env::Empty);

    let mut mem = Memory::new();
    let init: HashMap<String, usize> = prog
        .func
        .into_iter()
        .map(|f| (f.name, mem.alloc(Value::Func(f.args, Arc::new(f.body)))))
        .collect();
    let env = env.extend(init);

    let entry = env.lookup("main").unwrap();
    let expr = match mem.load(entry) {
        Value::Func(items, expr) => {
            assert!(items.is_empty());
            expr.clone()
        }
        _ => panic!("runtime error"),
    };
    
    eval_expr(env, &mut mem, &expr)
}
