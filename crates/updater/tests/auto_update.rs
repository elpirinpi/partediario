//! Prueba de integración del auto-actualizador contra un servidor HTTP local.
//!
//! Se simula la respuesta de GitHub Releases (JSON), la descarga del ZIP y su
//! checksum SHA-256, sin depender de la red ni de una release real.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};

use parte_diario_updater::{check_for_update_at, download_and_prepare_to_dir, UpdaterError};
use sha2::{Digest, Sha256};

const ZIP_NAME: &str = "parte-diario-windows-x64.zip";
const RELEASE_PATH: &str = "/repos/test/parte-diario/releases/latest";

struct Fixture {
    release_url: String,
    _server: JoinHandle<()>,
}

fn spawn_server(listener: TcpListener, routes: HashMap<String, Vec<u8>>) -> JoinHandle<()> {
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };

            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let read = stream.read(&mut chunk).unwrap_or(0);
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
                if request.len() >= 16_384 {
                    break;
                }
            }

            let request = String::from_utf8_lossy(&request);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/");

            let (status, body): (&str, &[u8]) = match routes.get(path) {
                Some(body) => ("200 OK", body.as_slice()),
                None => ("404 Not Found", b""),
            };
            let header = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: application/octet-stream\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(body);
        }
    })
}

fn release_json(base: &str, tag: &str, include_zip: bool) -> Vec<u8> {
    let mut assets = Vec::new();
    if include_zip {
        assets.push(serde_json::json!({
            "name": ZIP_NAME,
            "browser_download_url": format!("{base}/{ZIP_NAME}")
        }));
        assets.push(serde_json::json!({
            "name": format!("{ZIP_NAME}.sha256"),
            "browser_download_url": format!("{base}/{ZIP_NAME}.sha256")
        }));
    }
    serde_json::to_vec(&serde_json::json!({
        "tag_name": tag,
        "html_url": format!("{base}/release"),
        "body": "Notas de la versión de prueba",
        "assets": assets
    }))
    .unwrap()
}

fn start_server(tag: &str, zip: Option<Vec<u8>>, checksum: Option<String>) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let base = format!("http://{address}");
    let release_url = format!("{base}{RELEASE_PATH}");

    let mut routes = HashMap::new();
    routes.insert(
        RELEASE_PATH.to_string(),
        release_json(&base, tag, zip.is_some()),
    );
    if let Some(zip) = zip {
        routes.insert(format!("/{ZIP_NAME}"), zip);
        if let Some(checksum) = checksum {
            routes.insert(format!("/{ZIP_NAME}.sha256"), checksum.into_bytes());
        }
    }

    Fixture {
        release_url,
        _server: spawn_server(listener, routes),
    }
}

fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }
    buffer.into_inner()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[test]
fn detecta_actualizacion_y_descarga_paquete_valido() {
    let exe = b"contenido del ejecutable de prueba".to_vec();
    let zip = make_zip(&[("parte-diario.exe", &exe)]);
    let checksum = format!("{}  {ZIP_NAME}\n", sha256_hex(&zip));
    let fixture = start_server("v2.1.0", Some(zip), Some(checksum));

    let info = check_for_update_at(&fixture.release_url, "2.0.0")
        .expect("la consulta no debe fallar")
        .expect("debe detectar una versión nueva");

    assert_eq!(info.current_version, "2.0.0");
    assert_eq!(info.latest_version, "2.1.0");
    assert_eq!(info.asset_name, ZIP_NAME);
    assert!(info.checksum_url.is_some());
    assert_eq!(info.notes, "Notas de la versión de prueba");

    let dir = tempfile::tempdir().unwrap();
    let prepared = download_and_prepare_to_dir(dir.path(), &info)
        .expect("la descarga y verificación deben funcionar");

    assert_eq!(
        prepared,
        dir.path().join("staging").join("parte-diario.exe")
    );
    assert_eq!(fs::read(&prepared).unwrap(), exe);
    assert!(dir.path().join(ZIP_NAME).exists());
}

#[test]
fn no_hay_actualizacion_si_la_version_actual_es_la_ultima() {
    let fixture = start_server("v2.0.0", None, None);
    let result = check_for_update_at(&fixture.release_url, "2.0.0").unwrap();
    assert!(result.is_none());
}

#[test]
fn rechaza_paquete_con_checksum_incorrecto() {
    let zip = make_zip(&[("parte-diario.exe", b"binario")]);
    let checksum = format!("{}  {ZIP_NAME}\n", "0".repeat(64));
    let fixture = start_server("v2.1.0", Some(zip), Some(checksum));

    let info = check_for_update_at(&fixture.release_url, "2.0.0")
        .unwrap()
        .expect("debe detectar actualización");

    let dir = tempfile::tempdir().unwrap();
    let error = download_and_prepare_to_dir(dir.path(), &info).unwrap_err();

    assert!(
        matches!(error, UpdaterError::ChecksumMismatch),
        "error inesperado: {error:?}"
    );
    assert!(!dir.path().join("staging").exists());
    assert!(!dir.path().join(ZIP_NAME).exists());
}

#[test]
fn rechaza_zip_sin_ejecutable() {
    let zip = make_zip(&[("otro-archivo.txt", b"sin binario")]);
    let checksum = format!("{}  {ZIP_NAME}\n", sha256_hex(&zip));
    let fixture = start_server("v2.1.0", Some(zip), Some(checksum));

    let info = check_for_update_at(&fixture.release_url, "2.0.0")
        .unwrap()
        .expect("debe detectar actualización");

    let dir = tempfile::tempdir().unwrap();
    let error = download_and_prepare_to_dir(dir.path(), &info).unwrap_err();

    match error {
        UpdaterError::InvalidPackage(message) => {
            assert!(
                message.contains("parte-diario.exe"),
                "mensaje inesperado: {message}"
            );
        }
        other => panic!("error inesperado: {other:?}"),
    }
}

#[test]
fn falla_si_el_release_no_incluye_zip() {
    let fixture = start_server("v2.1.0", None, None);
    let error = check_for_update_at(&fixture.release_url, "2.0.0").unwrap_err();

    match error {
        UpdaterError::InvalidPackage(message) => {
            assert!(message.contains("zip"), "mensaje inesperado: {message}");
        }
        other => panic!("error inesperado: {other:?}"),
    }
}
