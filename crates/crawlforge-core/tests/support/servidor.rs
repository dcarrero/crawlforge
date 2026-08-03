//! Servidor HTTP/1.1 mínimo para los tests de integración.
//!
//! # Por qué un servidor propio y no `axum` ni `wiremock`
//!
//! Lo que estos tests necesitan del servidor cabe en una frase: devolver un código de estado, una
//! cabecera `Location`, un `Content-Type` y, cuando toca, tardar. Nada de rutas con parámetros,
//! extractores ni middleware. Un juguete de HTTP/1.1 sobre `tokio::net::TcpListener` —que ya es
//! dependencia del core— da control exacto sobre esas cuatro cosas y no añade ni una caja al
//! árbol de dependencias del proyecto.
//!
//! # Lo que sí garantiza
//!
//! - Puerto libre asignado por el sistema (`127.0.0.1:0`), así que dos tests en paralelo no
//!   chocan.
//! - Una respuesta fija por ruta, y 404 para todo lo demás: `robots.txt` y los sitemaps de las
//!   rutas convencionales que el motor pide por su cuenta responden como en un sitio que no los
//!   tiene, sin que el test tenga que declararlos.
//! - `Connection: close` en cada respuesta y cierre del socket: no quedan conexiones vivas
//!   cuando el test termina.
//! - Se apaga al soltarlo. La tarea que acepta conexiones se aborta en `Drop`, y con ella el
//!   `JoinSet` de las conexiones en curso.
//! - Cuenta las peticiones recibidas por ruta, que es como se comprueba lo que el motor pide de
//!   verdad —y lo que no pide.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Lo que el servidor devuelve para una ruta.
#[derive(Clone, Debug)]
pub struct Respuesta {
    pub status: u16,
    /// Cabeceras adicionales. `Content-Length` y `Connection` las pone el servidor.
    pub headers: Vec<(String, String)>,
    pub body: String,
    /// Cuánto se espera antes de responder. Es lo que provoca `HTTP-SLOW-RESPONSE`.
    pub retardo: Duration,
    /// Valor exacto de `Authorization` sin el cual la ruta responde 401, como un staging
    /// protegido con Basic Auth. `None` = ruta pública.
    pub autorizacion_requerida: Option<String>,
}

impl Respuesta {
    fn nueva(status: u16, content_type: &str, body: impl Into<String>) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".to_string(), content_type.to_string())],
            body: body.into(),
            retardo: Duration::ZERO,
            autorizacion_requerida: None,
        }
    }

    /// 200 con el HTML que se le pase.
    pub fn html(body: impl Into<String>) -> Self {
        Self::nueva(200, "text/html; charset=utf-8", body)
    }

    /// 200 en texto plano: lo que sirve un `robots.txt`.
    ///
    /// El `allow`: este módulo se compila una vez por binario de test y no todos usan toda la
    /// API. Mismo motivo en el resto de `allow(dead_code)` del fichero.
    #[allow(dead_code)]
    pub fn texto(body: impl Into<String>) -> Self {
        Self::nueva(200, "text/plain; charset=utf-8", body)
    }

    /// 200 en XML: lo que sirve un sitemap.
    #[allow(dead_code)]
    pub fn xml(body: impl Into<String>) -> Self {
        Self::nueva(200, "application/xml", body)
    }

    /// Una página completa: el motor solo evalúa reglas de página sobre documentos con `<html>`,
    /// así que hasta el 500 de un test necesita un cuerpo de verdad.
    pub fn pagina(titulo: &str, cuerpo: &str) -> Self {
        Self::html(format!(
            "<!DOCTYPE html><html lang=\"es\"><head><meta charset=\"utf-8\">\
             <title>{titulo}</title>\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
             </head><body><main><h1>{titulo}</h1>{cuerpo}</main></body></html>"
        ))
    }

    /// Redirección con `Location`. El motor no la sigue: cada salto es una fila del informe.
    #[allow(dead_code)]
    pub fn redirige(status: u16, hacia: &str) -> Self {
        let mut r = Self::nueva(status, "text/html; charset=utf-8", String::new());
        r.headers.push(("Location".to_string(), hacia.to_string()));
        r
    }

    /// Error con cuerpo HTML, como el de cualquier servidor real.
    ///
    /// El cuerpo no es decoración: el motor solo construye el `PageContext` —y con él evalúa las
    /// reglas de página, `HTTP-5XX` entre ellas— cuando la respuesta trae HTML y no viene vacía.
    /// Un 500 con el cuerpo a cero no dispararía nada.
    pub fn error(status: u16) -> Self {
        let mut r = Self::pagina(&format!("Error {status}"), "<p>Algo ha ido mal.</p>");
        r.status = status;
        r
    }

    /// La misma respuesta, pero tardando.
    #[allow(dead_code)]
    pub fn con_retardo(mut self, retardo: Duration) -> Self {
        self.retardo = retardo;
        self
    }

    /// La misma respuesta, pero solo con la cabecera `Authorization` exacta; sin ella, 401.
    /// Es cómo se monta un staging protegido con Basic Auth en los tests.
    #[allow(dead_code)]
    pub fn exigiendo_autorizacion(mut self, valor: &str) -> Self {
        self.autorizacion_requerida = Some(valor.to_string());
        self
    }

    /// El 401 con el que responde una ruta protegida, con su `WWW-Authenticate` de rigor.
    fn no_autorizada() -> Self {
        let mut r = Self::pagina("401 Unauthorized", "<p>Authentication required.</p>");
        r.status = 401;
        r.headers
            .push(("WWW-Authenticate".to_string(), "Basic realm=\"staging\"".to_string()));
        r
    }

    /// Lo que se devuelve para una ruta no declarada.
    fn no_encontrada() -> Self {
        Self::error(404)
    }

    fn serializar(&self) -> Vec<u8> {
        let mut salida = format!("HTTP/1.1 {} {}\r\n", self.status, motivo(self.status));
        for (nombre, valor) in &self.headers {
            salida.push_str(&format!("{nombre}: {valor}\r\n"));
        }
        salida.push_str(&format!("Content-Length: {}\r\n", self.body.len()));
        salida.push_str("Connection: close\r\n\r\n");
        let mut bytes = salida.into_bytes();
        bytes.extend_from_slice(self.body.as_bytes());
        bytes
    }
}

/// Frase del código de estado. Ningún cliente decide nada con ella, pero un volcado de red
/// ilegible cuesta media hora de depuración.
fn motivo(status: u16) -> &'static str {
    match status {
        200 => "OK",
        301 => "Moved Permanently",
        302 => "Found",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        401 => "Unauthorized",
        404 => "Not Found",
        410 => "Gone",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

type Rutas = Arc<HashMap<String, Respuesta>>;
type Contador = Arc<Mutex<HashMap<String, usize>>>;
/// La cabecera `Authorization` que trajo cada petición, por ruta y en orden de llegada.
/// `None` = la petición no la llevaba. Es cómo se afirma que una credencial viajó — o que no.
type Autorizaciones = Arc<Mutex<HashMap<String, Vec<Option<String>>>>>;

/// Un servidor de pruebas vivo. Se apaga al soltarlo.
pub struct ServidorDePruebas {
    puerto: u16,
    #[allow(dead_code)]
    peticiones: Contador,
    autorizaciones: Autorizaciones,
    tareas: Vec<tokio::task::JoinHandle<()>>,
}

impl ServidorDePruebas {
    /// Arranca en `127.0.0.1` con un puerto libre y sirve el mapa de rutas dado.
    pub async fn arrancar(rutas: &[(&str, Respuesta)]) -> Self {
        Self::arrancar_interno(rutas, false).await
    }

    /// Arranca con rutas que necesitan conocer el puerto antes de escribirse.
    ///
    /// Existe por los sitemaps: el protocolo exige `<loc>` absolutos, y el puerto no se sabe
    /// hasta abrir el socket. Este constructor abre primero y construye las rutas después.
    #[allow(dead_code)]
    pub async fn arrancar_con_puerto(
        rutas: impl FnOnce(u16) -> Vec<(String, Respuesta)>,
    ) -> Self {
        let escucha = TcpListener::bind("127.0.0.1:0").await.expect("abrir el puerto de pruebas");
        let puerto = escucha.local_addr().expect("puerto asignado").port();

        let mapa: Rutas = Arc::new(rutas(puerto).into_iter().collect());
        let peticiones: Contador = Arc::new(Mutex::new(HashMap::new()));
        let autorizaciones: Autorizaciones = Arc::new(Mutex::new(HashMap::new()));
        let tareas = vec![{
            let mapa = Arc::clone(&mapa);
            let peticiones = Arc::clone(&peticiones);
            let autorizaciones = Arc::clone(&autorizaciones);
            tokio::spawn(aceptar(escucha, mapa, peticiones, autorizaciones))
        }];

        Self { puerto, peticiones, autorizaciones, tareas }
    }

    /// [`Self::arrancar_con_puerto`] y [`Self::arrancar_como_otro_host`] a la vez: rutas que
    /// necesitan conocer el puerto —enlaces absolutos a `localhost`— en un servidor que también
    /// atiende como ese otro host.
    #[allow(dead_code)]
    pub async fn arrancar_como_otro_host_con_puerto(
        rutas: impl FnOnce(u16) -> Vec<(String, Respuesta)>,
    ) -> Self {
        let escucha = TcpListener::bind("127.0.0.1:0").await.expect("abrir el puerto de pruebas");
        let puerto = escucha.local_addr().expect("puerto asignado").port();

        let mut escuchas = vec![escucha];
        if let Ok(v6) = TcpListener::bind(("::1", puerto)).await {
            escuchas.push(v6);
        }

        let mapa: Rutas = Arc::new(rutas(puerto).into_iter().collect());
        let peticiones: Contador = Arc::new(Mutex::new(HashMap::new()));
        let autorizaciones: Autorizaciones = Arc::new(Mutex::new(HashMap::new()));
        let tareas = escuchas
            .into_iter()
            .map(|escucha| {
                let mapa = Arc::clone(&mapa);
                let peticiones = Arc::clone(&peticiones);
                let autorizaciones = Arc::clone(&autorizaciones);
                tokio::spawn(aceptar(escucha, mapa, peticiones, autorizaciones))
            })
            .collect();

        Self { puerto, peticiones, autorizaciones, tareas }
    }

    /// Igual, pero además intenta escuchar en `[::1]` con el mismo puerto.
    ///
    /// Es lo que permite usar `localhost` como **otro host** sin salir de la máquina: el motor
    /// compara hosts por su nombre (`normalize::is_internal`), así que `localhost` y `127.0.0.1`
    /// son dominios distintos aunque acaben en el mismo sitio. Como `localhost` puede resolver
    /// primero a `::1`, se escuchan las dos familias; si la segunda no se puede abrir, el cliente
    /// reintenta por IPv4 y el test sigue siendo válido.
    #[allow(dead_code)]
    pub async fn arrancar_como_otro_host(rutas: &[(&str, Respuesta)]) -> Self {
        Self::arrancar_interno(rutas, true).await
    }

    async fn arrancar_interno(rutas: &[(&str, Respuesta)], tambien_ipv6: bool) -> Self {
        let mapa: Rutas = Arc::new(
            rutas.iter().map(|(ruta, r)| ((*ruta).to_string(), r.clone())).collect(),
        );
        let peticiones: Contador = Arc::new(Mutex::new(HashMap::new()));
        let autorizaciones: Autorizaciones = Arc::new(Mutex::new(HashMap::new()));

        let escucha = TcpListener::bind("127.0.0.1:0").await.expect("abrir el puerto de pruebas");
        let puerto = escucha.local_addr().expect("puerto asignado").port();

        let mut escuchas = vec![escucha];
        if tambien_ipv6 {
            if let Ok(v6) = TcpListener::bind(("::1", puerto)).await {
                escuchas.push(v6);
            }
        }

        let tareas = escuchas
            .into_iter()
            .map(|escucha| {
                let mapa = Arc::clone(&mapa);
                let peticiones = Arc::clone(&peticiones);
                let autorizaciones = Arc::clone(&autorizaciones);
                tokio::spawn(aceptar(escucha, mapa, peticiones, autorizaciones))
            })
            .collect();

        Self { puerto, peticiones, autorizaciones, tareas }
    }

    /// URL base del sitio, con la barra final.
    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}/", self.puerto)
    }

    /// URL absoluta de una ruta, tal como la vería el informe.
    pub fn url(&self, ruta: &str) -> String {
        format!("http://127.0.0.1:{}{ruta}", self.puerto)
    }

    /// Misma URL, pero nombrando al host `localhost`: para el motor es otro dominio.
    pub fn url_como_otro_host(&self, ruta: &str) -> String {
        format!("http://localhost:{}{ruta}", self.puerto)
    }

    /// Cuántas veces se ha pedido una ruta.
    #[allow(dead_code)]
    pub fn peticiones(&self, ruta: &str) -> usize {
        self.peticiones
            .lock()
            .expect("el contador de peticiones no debería envenenarse")
            .get(ruta)
            .copied()
            .unwrap_or(0)
    }

    /// La cabecera `Authorization` que trajo cada petición a una ruta, en orden de llegada.
    /// Una petición sin la cabecera aparece como `None`: la ausencia también se afirma.
    #[allow(dead_code)]
    pub fn autorizaciones(&self, ruta: &str) -> Vec<Option<String>> {
        self.autorizaciones
            .lock()
            .expect("el registro de autorizaciones no debería envenenarse")
            .get(ruta)
            .cloned()
            .unwrap_or_default()
    }
}

impl Drop for ServidorDePruebas {
    fn drop(&mut self) {
        // Abortar la tarea que acepta suelta también su `JoinSet`, y con él las conexiones en
        // curso: al terminar el test no queda nada escuchando ni ningún hilo colgado.
        for tarea in &self.tareas {
            tarea.abort();
        }
    }
}

async fn aceptar(
    escucha: TcpListener,
    rutas: Rutas,
    peticiones: Contador,
    autorizaciones: Autorizaciones,
) {
    let mut conexiones = tokio::task::JoinSet::new();
    loop {
        let Ok((stream, _)) = escucha.accept().await else {
            return;
        };
        // Las conexiones ya terminadas se recogen aquí: sin esto el `JoinSet` crecería durante
        // todo el rastreo.
        while conexiones.try_join_next().is_some() {}
        conexiones.spawn(atender(
            stream,
            Arc::clone(&rutas),
            Arc::clone(&peticiones),
            Arc::clone(&autorizaciones),
        ));
    }
}

async fn atender(
    mut stream: TcpStream,
    rutas: Rutas,
    peticiones: Contador,
    autorizaciones: Autorizaciones,
) {
    let Some((objetivo, autorizacion)) = leer_peticion(&mut stream).await else {
        return;
    };

    if let Ok(mut contador) = peticiones.lock() {
        *contador.entry(objetivo.clone()).or_insert(0) += 1;
    }
    if let Ok(mut registro) = autorizaciones.lock() {
        registro.entry(objetivo.clone()).or_default().push(autorizacion.clone());
    }

    let respuesta = rutas.get(&objetivo).cloned().unwrap_or_else(Respuesta::no_encontrada);
    // Una ruta protegida responde 401 salvo que la petición traiga la `Authorization` exacta,
    // como haría el Basic Auth de un staging real.
    let respuesta = match &respuesta.autorizacion_requerida {
        Some(esperada) if autorizacion.as_deref() != Some(esperada.as_str()) => {
            Respuesta::no_autorizada()
        }
        _ => respuesta,
    };
    if !respuesta.retardo.is_zero() {
        tokio::time::sleep(respuesta.retardo).await;
    }

    let _ = stream.write_all(&respuesta.serializar()).await;
    let _ = stream.flush().await;
    let _ = stream.shutdown().await;
}

/// Lee la petición hasta la línea en blanco y devuelve su objetivo (`/ruta?consulta`) y su
/// cabecera `Authorization`, si la traía.
///
/// No se interpreta nada más: el motor solo hace `GET`, y un servidor de pruebas que valide
/// cabeceras solo puede fallar de formas que no enseñan nada sobre el motor. `Authorization`
/// es la excepción con propósito: es lo que permite montar un staging protegido y afirmar a
/// qué host viajó una credencial — y a cuál no.
async fn leer_peticion(stream: &mut TcpStream) -> Option<(String, Option<String>)> {
    let mut crudo = Vec::new();
    let mut buffer = [0u8; 1024];

    while !crudo.windows(4).any(|v| v == b"\r\n\r\n") {
        let leidos = stream.read(&mut buffer).await.ok()?;
        if leidos == 0 {
            return None;
        }
        crudo.extend_from_slice(&buffer[..leidos]);
        if crudo.len() > 64 * 1024 {
            return None;
        }
    }

    let texto = String::from_utf8_lossy(&crudo);
    let primera = texto.lines().next()?;
    let mut partes = primera.split_whitespace();
    let _metodo = partes.next()?;
    let objetivo = partes.next()?.to_string();

    let autorizacion = texto.lines().skip(1).take_while(|l| !l.is_empty()).find_map(|linea| {
        let (nombre, valor) = linea.split_once(':')?;
        nombre
            .trim()
            .eq_ignore_ascii_case("authorization")
            .then(|| valor.trim().to_string())
    });

    Some((objetivo, autorizacion))
}
