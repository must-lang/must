#[salsa::tracked(debug)]
pub struct Span<'db> {
    #[tracked]
    pub start_byte: usize,
    #[tracked]
    pub end_byte: usize,
}
