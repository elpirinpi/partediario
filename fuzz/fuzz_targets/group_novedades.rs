#![no_main]

use std::collections::HashSet;

use libfuzzer_sys::fuzz_target;
use parte_diario_parser::grouping::group_novedades;
use parte_diario_parser::parser::{parse_chat_txt, AttachmentKind};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    let chat = parse_chat_txt(text);

    // Simula el contenido multimedia del .zip: los adjuntos de imagen que
    // aparecen en el chat se consideran presentes.
    let mut media_names: HashSet<String> = HashSet::new();
    for message in &chat.messages {
        if let Some(attachment) = &message.attachment {
            if attachment.kind == AttachmentKind::Image {
                media_names.insert(attachment.filename.to_lowercase());
            }
        }
    }

    let _ = group_novedades(&chat, &media_names);
});
