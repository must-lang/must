use crate::tp::Type;

pub fn get_size(tp: &Type) -> usize {
    match tp {
        Type::Error => panic!("{tp:?}"),
        Type::Int => 1,
        Type::Tuple(tps) => tps.iter().map(get_size).sum(),
        Type::Bool => 1,
        Type::Array(n, tp) => n * get_size(tp),
        Type::Fn(_, _) => 1,
        Type::Ptr { .. } => 1,
        Type::Slice { .. } => 2,
    }
}
