//! Los módulos de la CLI que se compilan y se prueban por separado del binario.
//!
//! Existe por una razón concreta: `src/main.rs` es un binario, y un módulo que solo se declara
//! ahí no se compila —ni se testea— hasta que alguien escribe su `mod`. `diff` se entregó antes
//! de que su subcomando estuviera cableado, así que vive aquí para que
//! `cargo test -p crawlforge-cli` lo ejecute de verdad.
//!
//! **Al cablear `crawlforge diff` en `main.rs`, la línea es `use crawlforge_cli::diff;`** —no
//! `mod diff;`—. Con `mod` el módulo se compilaría dos veces, una por cada objetivo, y en el
//! binario todo lo que el subcomando no llame saldría como código muerto, que con
//! `clippy -D warnings` rompe la compilación.

pub mod audit_report;
pub mod diff;
pub mod i18n;
pub mod inspect;
pub mod store_check;
pub mod xlsx;
