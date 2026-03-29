/// OPUS MT560 English–Somali parallel corpus on Hugging Face.
///
/// JW300 `en-so` is no longer publicly available on OPUS. This release
/// provides ~161K English–Somali pairs with religious and educational Somali
/// text.
const DATASET_REPO: &str = "michsethowusu/english-somali_sentence-pairs_mt560";
const TEXT_COLUMN: &str = "som";
pub const SOURCE_TAG: &str = "mt560";

pub fn mt560_train_shards() -> Vec<String> {
    vec!["data/train-00000-of-00001.parquet".to_string()]
}

pub fn dataset_repo() -> &'static str {
    DATASET_REPO
}

pub fn text_column() -> &'static str {
    TEXT_COLUMN
}

pub fn source_url() -> String {
    format!(
        "https://huggingface.co/datasets/{DATASET_REPO} (Somali column: {TEXT_COLUMN})"
    )
}
