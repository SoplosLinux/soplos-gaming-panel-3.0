// The few words the tray menu needs, which lives outside the window and so
// cannot use the dictionaries of the interface.

/// The code of a locale of the session, cut down to the language: "es_ES.UTF-8"
/// is "es". Anything the program does not speak comes back as English.
fn code_of(value: &str) -> String {
    let language = value
        .split(['_', '.', '@'])
        .next()
        .unwrap_or("")
        .to_lowercase();
    match language.as_str() {
        "es" | "de" | "fr" | "it" | "pt" | "ro" | "ru" | "en" => language,
        _ => "en".to_string(),
    }
}

/// The language of the desktop, which is what the program follows when no
/// language has been chosen in the settings. It is read from the same place
/// the window reads it: the locale of the session.
pub fn desktop() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
        .map(|value| code_of(&value))
        .unwrap_or_else(|| "en".to_string())
}

/// The language the tray speaks: the chosen one, else the one of the desktop.
pub fn language(settings: &crate::config::Settings) -> String {
    settings.language.clone().unwrap_or_else(desktop)
}

pub fn t(language: &str, key: &str) -> &'static str {
    match (language, key) {
        ("es", "show") => "Mostrar el panel",
        ("es", "hide") => "Ocultar el panel",
        ("es", "hud-on") => "Activar el overlay",
        ("es", "hud-off") => "Desactivar el overlay",
        ("es", "settings") => "Ajustes…",
        ("es", "quit") => "Salir",
        ("de", "show") => "Panel anzeigen",
        ("de", "hide") => "Panel ausblenden",
        ("de", "hud-on") => "Overlay einschalten",
        ("de", "hud-off") => "Overlay ausschalten",
        ("de", "settings") => "Einstellungen…",
        ("de", "quit") => "Beenden",
        ("fr", "show") => "Afficher le panneau",
        ("fr", "hide") => "Masquer le panneau",
        ("fr", "hud-on") => "Activer l’overlay",
        ("fr", "hud-off") => "Désactiver l’overlay",
        ("fr", "settings") => "Réglages…",
        ("fr", "quit") => "Quitter",
        ("it", "show") => "Mostra il pannello",
        ("it", "hide") => "Nascondi il pannello",
        ("it", "hud-on") => "Attiva l’overlay",
        ("it", "hud-off") => "Disattiva l’overlay",
        ("it", "settings") => "Impostazioni…",
        ("it", "quit") => "Esci",
        ("pt", "show") => "Mostrar o painel",
        ("pt", "hide") => "Ocultar o painel",
        ("pt", "hud-on") => "Ativar o overlay",
        ("pt", "hud-off") => "Desativar o overlay",
        ("pt", "settings") => "Definições…",
        ("pt", "quit") => "Sair",
        ("ro", "show") => "Arată panoul",
        ("ro", "hide") => "Ascunde panoul",
        ("ro", "hud-on") => "Activează suprapunerea",
        ("ro", "hud-off") => "Dezactivează suprapunerea",
        ("ro", "settings") => "Setări…",
        ("ro", "quit") => "Ieșire",
        ("ru", "show") => "Показать панель",
        ("ru", "hide") => "Скрыть панель",
        ("ru", "hud-on") => "Включить оверлей",
        ("ru", "hud-off") => "Выключить оверлей",
        ("ru", "settings") => "Настройки…",
        ("ru", "quit") => "Выход",
        ("en", "show") => "Show the panel",
        ("en", "hide") => "Hide the panel",
        ("en", "hud-on") => "Turn the overlay on",
        ("en", "hud-off") => "Turn the overlay off",
        ("en", "settings") => "Settings…",
        ("en", "quit") => "Quit",
        ("en", _) => "",
        (_, "show") => "Show the panel",
        (_, "hide") => "Hide the panel",
        (_, "hud-on") => "Turn the overlay on",
        (_, "hud-off") => "Turn the overlay off",
        (_, "settings") => "Settings…",
        _ => "Quit",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_locale_of_the_session_gives_the_language() {
        assert_eq!(code_of("es_ES.UTF-8"), "es");
        assert_eq!(code_of("de_DE.utf8"), "de");
        assert_eq!(code_of("pt_BR"), "pt");
        assert_eq!(code_of("en_US.UTF-8"), "en");
    }

    #[test]
    fn a_language_the_program_does_not_speak_falls_back_to_english() {
        assert_eq!(code_of("ja_JP.UTF-8"), "en");
        assert_eq!(code_of(""), "en");
    }

    fn keys_of(value: &serde_json::Value, path: &str, into: &mut BTreeSet<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, inner) in map {
                    let full = if path.is_empty() { key.clone() } else { format!("{path}.{key}") };
                    keys_of(inner, &full, into);
                }
            }
            _ => {
                into.insert(path.to_string());
            }
        }
    }

    /// The dictionaries of the window are read by the same keys in every
    /// language; a key that only exists in one of them is a missing word.
    #[test]
    fn every_language_has_the_same_words() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/i18n/locales");
        let mut expected: Option<(String, BTreeSet<String>)> = None;
        for language in ["en", "es", "de", "fr", "it", "pt", "ro", "ru"] {
            let file = root.join(format!("{language}.json"));
            let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            let value: serde_json::Value = serde_json::from_str(&text).expect("json válido");
            let mut keys = BTreeSet::new();
            keys_of(&value, "", &mut keys);
            match &expected {
                None => expected = Some((language.to_string(), keys)),
                Some((first, reference)) => {
                    let missing: Vec<&String> = reference.difference(&keys).collect();
                    let extra: Vec<&String> = keys.difference(reference).collect();
                    assert!(
                        missing.is_empty() && extra.is_empty(),
                        "{language} no coincide con {first}: faltan {missing:?}, sobran {extra:?}"
                    );
                }
            }
        }
    }
}
