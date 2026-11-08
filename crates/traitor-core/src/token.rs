const BYTES_PER_TOKEN: usize = 3;

pub fn estimate_token_count(text: &str) -> usize {
    text.len().div_ceil(BYTES_PER_TOKEN)
}
