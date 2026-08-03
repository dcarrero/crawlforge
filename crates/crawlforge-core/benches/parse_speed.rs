//! Medición del parseo aislado, sin red. `cargo bench -p crawlforge-core` no está configurado
//! con criterion todavía; esto se ejecuta como binario de test.
fn main() {
    let path = std::env::args().nth(1).expect("uso: parse_speed <fichero.html>");
    let html = std::fs::read(&path).expect("leer HTML");
    let iterations = 200;

    // Calentamiento.
    for _ in 0..10 {
        std::hint::black_box(crawlforge_core::parse::parse_html(&html, false));
    }

    let started = std::time::Instant::now();
    for _ in 0..iterations {
        std::hint::black_box(crawlforge_core::parse::parse_html(&html, false));
    }
    let elapsed = started.elapsed();

    let mb = (html.len() as f64 * iterations as f64) / (1024.0 * 1024.0);
    println!("{iterations} páginas de {} KB", html.len() / 1024);
    println!("  total     {:.2} s", elapsed.as_secs_f64());
    println!("  por pág.  {:.2} ms", elapsed.as_secs_f64() * 1000.0 / iterations as f64);
    println!("  ritmo     {:.0} MB/s", mb / elapsed.as_secs_f64());
    println!("  páginas/s {:.0}", iterations as f64 / elapsed.as_secs_f64());
}
