// Library crate root — exports modules for benchmarks and external access.
// The binary crate (main.rs) uses its own `mod lib;` declaration.
pub mod lib {
    #[path = "constants.rs"]
    pub mod constants;
    #[path = "spatial/mod.rs"]
    pub mod spatial;
}
