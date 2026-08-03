//! Frontera FFI: UniFFI (Swift) y `extern "C"` (C#).
//!
//! Los datos NO cruzan este puente: viajan por el fichero SQLite. Aqui solo pasan
//! ordenes de control y un `ProgressSnapshot` plano. Si esta superficie supera las
//! quince funciones, es que algo se esta marshallando. Ver `docs/01-ARQUITECTURA.md` SS2 y SS4.
//!
//! Ninguna funcion FFI es `async`. Ninguna hace panic: todo punto de entrada se envuelve
//! en `catch_unwind`. Lo consumirán las interfaces nativas.
