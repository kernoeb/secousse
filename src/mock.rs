use std::sync::OnceLock;

const MOCK_CHAT_RATE: u64 = 30;
const MOCK_EMOTE_SIZE_PX: u32 = 24;

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("SECOUSSE_MOCK").is_ok())
}

pub fn chat_rate() -> u64 {
    MOCK_CHAT_RATE
}

pub fn emote_url_for_id(emote_id: &str, scale: &str) -> String {
    if enabled()
        && let Some(mock_id) = emote_id.strip_prefix("mock-")
    {
        let label = urlencoding::encode(mock_id);
        return format!(
            "https://placehold.co/{MOCK_EMOTE_SIZE_PX}x{MOCK_EMOTE_SIZE_PX}?text={label}"
        );
    }

    format!("https://static-cdn.jtvnw.net/emoticons/v2/{emote_id}/default/dark/{scale}")
}
