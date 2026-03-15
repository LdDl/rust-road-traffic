// Library crate root for exporting modules in benchmarks and possible external access.
// WARNING: binary crate itself (main.rs) uses its own `mod lib;` declaration.
// @todo: will be changed in future
pub mod lib {
    #[path = "constants.rs"]
    pub mod constants;
    #[path = "spatial/mod.rs"]
    pub mod spatial;
}
