/// Known shard layout for `allenai/c4` config `so`.
///
/// The Hugging Face tree API paginates at 1000 entries and Somali files
/// (`c4-so.*`) are not included in the first page, so we enumerate shards
/// directly instead of listing the repo.
pub fn mc4_so_train_shards() -> Vec<String> {
    (0..64)
        .map(|index| format!("multilingual/c4-so.tfrecord-{index:05}-of-00064.json.gz"))
        .collect()
}
