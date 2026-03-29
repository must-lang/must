use crate::typecheck::SType;

pub fn get_size(tp: &SType) -> usize {
    match tp {
        SType::Error => panic!("{tp:?}"),
        SType::Int => 1,
        SType::Tuple(stypes) => stypes.iter().map(|tp| get_size(tp)).sum(),
        SType::Bool => 1,
        SType::UnifVar(unif_var) => panic!(),
        SType::Array(n, stype) => n * get_size(stype),
        SType::Fn(stypes, stype) => todo!(),
        SType::Ptr { tp, is_mut } => 1,
    }
}
