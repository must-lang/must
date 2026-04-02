use std::collections::HashMap;

use salsa::Database;

use crate::{
    bytecode::{lower::LowerCtx, place::Place, value::Value},
    layout::get_size,
    parser::ast,
    typecheck::{self, FnSignature, InferenceResult, SType},
};

pub mod ir;
pub mod lower;
mod place;
mod value;

#[salsa::tracked]
pub fn compile<'db>(db: &'db dyn Database, prog: ast::File<'db>) -> ir::Prog {
    let InferenceResult {
        types,
        signatures,
        coercions,
    } = typecheck::check_file(db, prog);
    let mut funcs = HashMap::new();
    for def in prog.defs(db) {
        match def {
            ast::Def::FnDef(fn_def) => {
                if let None = fn_def.body(db)
                    && fn_def.ext(db)
                {
                    continue;
                }
                let sig = signatures.get(fn_def).unwrap();
                let (name, func) = lower_function(db, *fn_def, sig, &types, &coercions);
                funcs.insert(name, func);
            }
        }
    }
    ir::Prog { funcs }
}

pub fn lower_function<'db>(
    db: &'db dyn Database,
    ast_fn: ast::FnDef<'db>,
    sig: &FnSignature,
    types: &'db HashMap<ast::ExprId<'db>, typecheck::SType>,
    coercions: &'db HashMap<ast::ExprId<'db>, typecheck::Coercion>,
) -> (String, ir::Func) {
    let mut builder = ir::IrBuilder::new();

    let mut ctx = LowerCtx {
        db,
        scopes: vec![HashMap::new()],
        builder: &mut builder,
        types,
        coercions,
    };

    let ret_size = get_size(&sig.ret);
    let sret_place = if ret_size > 1 {
        // Allocate the first register (r0) to hold the hidden SRET pointer
        let sret_reg = ctx.builder.new_reg();
        Some((
            sret_reg,
            Place::DynamicPtr {
                base: sret_reg,
                offset: 0,
            },
        ))
    } else {
        None
    };

    let mut param_regs = Vec::new();
    for _ in &sig.args {
        param_regs.push(ctx.builder.new_reg());
    }

    let mut arg_id = 0;
    for ((pat, _), tp) in ast_fn.args(db).into_iter().zip(&sig.args) {
        let reg = param_regs[arg_id];

        let size = get_size(tp);

        let arg_val = if size == 1 {
            Value::LVal(Place::Reg(reg))
        } else {
            Value::LVal(Place::DynamicPtr {
                base: reg,
                offset: 0,
            })
        };

        ctx.lower_pat(pat, arg_val, None, tp);
        arg_id += 1;
    }

    let (reg, place) = sret_place.unwrap_or_else(|| {
        let reg = ctx.builder.new_reg();
        (reg, Place::Reg(reg))
    });

    ctx.lower_value(ast_fn.body(db).unwrap(), Some(place));
    builder.blocks[builder.current_block.0].terminator = ir::Terminator::Return(reg);

    (
        ast_fn.name(db).text(db).clone(),
        ir::Func {
            register_count: builder.next_reg,
            blocks: builder.blocks,
            stack_slots: builder.stack_slots,
        },
    )
}
