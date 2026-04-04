use std::collections::HashMap;

use salsa::Database;

use crate::{
    bytecode::{lower::LowerCtx, place::Place, value::Value},
    def_map::{self, FunctionId},
    layout::get_size,
    mod_tree::mod_tree,
    parser::{Crate, ast, func_ast},
    resolve::{FnSignature, func_signature},
    tp::Type,
    typecheck::{self, InferenceResult},
};

pub mod ir;
pub mod lower;
mod place;
mod value;

#[salsa::tracked]
pub fn compile<'db>(db: &'db dyn Database, c: Crate) -> Option<ir::Prog> {
    let tree = mod_tree(db, c);
    let mut funcs = HashMap::new();
    for module_id in tree.keys() {
        // 3. Get the DefMap for this module
        let def_map = def_map::module_def_map(db, *module_id)?;

        // 4. Look at everything defined in this module
        for f in def_map.functions.values() {
            let name = f.name(db);
            if let Some(ir) = lower_function(db, *f) {
                funcs.insert(name, ir);
            }
        }
    }
    Some(ir::Prog { funcs })
}

// #[salsa::tracked]
// pub fn compile<'db>(db: &'db dyn Database, prog: ast::File<'db>) -> ir::Prog {
//     let InferenceResult { types, coercions } = typecheck::check_file(db, prog);
//     let mut funcs = HashMap::new();
//     for def in prog.defs(db) {
//         match def {
//             ast::Def::FnDef(fn_def) => {
//                 if let None = fn_def.body(db)
//                     && fn_def.ext(db)
//                 {
//                     continue;
//                 }
//                 let sig = signatures.get(fn_def).unwrap();
//                 let (name, func) = lower_function(db, *fn_def, sig, &types, &coercions);
//                 funcs.insert(name, func);
//             }
//         }
//     }
//     ir::Prog { funcs }
// }

#[salsa::tracked]
pub fn lower_function<'db>(db: &'db dyn Database, f: FunctionId<'db>) -> Option<ir::Func> {
    let hir_fn = func_ast(db, f);

    if let None = hir_fn.body(db)
        && hir_fn.ext(db)
    {
        return None;
    }

    let mut builder = ir::IrBuilder::new();

    let InferenceResult {
        inferred_types: types,
        coercions,
    } = &typecheck::check_fn(db, f);

    let mut ctx = LowerCtx {
        db,
        scopes: vec![HashMap::new()],
        builder: &mut builder,
        types,
        coercions,
    };

    let sig = func_signature(db, f);

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

    for (arg_id, ((pat, _), tp)) in hir_fn.args(db).into_iter().zip(&sig.args).enumerate() {
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
    }

    let (reg, place) = sret_place.unwrap_or_else(|| {
        let reg = ctx.builder.new_reg();
        (reg, Place::Reg(reg))
    });

    ctx.lower_value(hir_fn.body(db).unwrap(), Some(place));
    builder.blocks[builder.current_block.0].terminator = ir::Terminator::Return(reg);

    Some(ir::Func {
        register_count: builder.next_reg,
        blocks: builder.blocks,
        stack_slots: builder.stack_slots,
    })
}
