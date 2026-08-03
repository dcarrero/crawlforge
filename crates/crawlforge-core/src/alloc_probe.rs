//! Sonda de asignaciones para tests de regresión de memoria. **Solo existe en tests.**
//!
//! Varios arreglos de rendimiento eliminan asignaciones que no cambian el resultado: no hay
//! nada observable desde fuera salvo la memoria. La única forma honesta de que sus tests de
//! regresión fallen sin el arreglo es contar las asignaciones. Nació en `parse.rs` (el sink
//! del rewriter y las copias por nodo de texto) y se compartió aquí cuando el despacho en
//! modo `list` necesitó la misma sonda (revisión 2026-08-01 §4.1).
//!
//! El contador es thread-local para que los demás tests, que corren en paralelo en otros
//! hilos, no lo contaminen. Consecuencia para quien mide: lo que asigna otro hilo —el hilo
//! escritor, un servidor de test— no cuenta; si el código medido debe correr en un runtime
//! de tokio, que sea `current_thread`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static BYTES_ASIGNADOS: Cell<u64> = const { Cell::new(0) };
    static NUM_ASIGNACIONES: Cell<u64> = const { Cell::new(0) };
}

struct AsignadorContado;

// `try_with` y no `with`: durante el desmontaje de un hilo la TLS ya no está disponible
// y el asignador global se sigue llamando; un pánico ahí abortaría el proceso.
unsafe impl GlobalAlloc for AsignadorContado {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = BYTES_ASIGNADOS.try_with(|c| c.set(c.get() + layout.size() as u64));
        let _ = NUM_ASIGNACIONES.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = BYTES_ASIGNADOS.try_with(|c| c.set(c.get() + new_size as u64));
        let _ = NUM_ASIGNACIONES.try_with(|c| c.set(c.get() + 1));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ASIGNADOR: AsignadorContado = AsignadorContado;

/// Ejecuta `f` y devuelve su resultado junto a (bytes asignados, número de asignaciones)
/// en este hilo durante la ejecución.
pub(crate) fn midiendo_asignaciones<R>(f: impl FnOnce() -> R) -> (R, u64, u64) {
    BYTES_ASIGNADOS.with(|c| c.set(0));
    NUM_ASIGNACIONES.with(|c| c.set(0));
    let r = f();
    (r, BYTES_ASIGNADOS.with(Cell::get), NUM_ASIGNACIONES.with(Cell::get))
}
