#![no_main]

use libfuzzer_sys::fuzz_target;
use parte_diario_parser::parser::parse_chat_txt;

fuzz_target!(|data: &[u8]| {
    // El parser espera texto UTF-8 (export de WhatsApp); se descartan los
    // bytes que no forman UTF-8 válido.
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = parse_chat_txt(text);
    }
});
