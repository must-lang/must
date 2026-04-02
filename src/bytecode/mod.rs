use std::collections::HashMap;

use salsa::Database;

use crate::{
    bytecode::{lower::LowerCtx, place::Place, value::Value},
    parser::ast,
    typecheck::{self, SType},
};

pub mod ir;
pub mod lower;
mod place;
mod value;

#[salsa::tracked]
pub fn compile<'db>(db: &'db dyn Database, prog: ast::File<'db>) -> ir::Prog {
    let types = typecheck::check_file(db, prog).types;
    let mut funcs = HashMap::new();
    for def in prog.defs(db) {
        match def {
            ast::Def::FnDef(fn_def) => {
                if let None = fn_def.body(db)
                    && fn_def.ext(db)
                {
                    continue;
                }
                let (name, func) = lower_function(db, *fn_def, &types);
                funcs.insert(name, func);
            }
        }
    }
    ir::Prog { funcs }
}

pub fn lower_function<'db>(
    db: &'db dyn Database,
    ast_fn: ast::FnDef<'db>,
    types: &'db HashMap<ast::ExprId<'db>, typecheck::SType>,
) -> (String, ir::Func) {
    let mut builder = ir::IrBuilder::new();

    let mut ctx = LowerCtx {
        db,
        scopes: vec![HashMap::new()],
        builder: &mut builder,
        types,
    };

    for (pat, _) in ast_fn.args(db) {
        let reg = ctx.builder.new_reg();
        // TODO: get proper types, sret etc
        ctx.lower_pat(pat, Value::LVal(Place::Reg(reg)), None, &SType::Int);
    }

    let res_reg = ctx.builder.new_reg();
    ctx.lower_value(ast_fn.body(db).unwrap(), Some(Place::Reg(res_reg)));

    builder.blocks[builder.current_block.0].terminator = ir::Terminator::Return(res_reg);

    (
        ast_fn.name(db).text(db).clone(),
        ir::Func {
            register_count: builder.next_reg,
            blocks: builder.blocks,
            stack_slots: builder.stack_slots,
        },
    )
}
