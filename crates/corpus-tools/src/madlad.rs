const DATASET_REPO: &str = "allenai/MADLAD-400";
const LANGUAGE: &str = "so";

/// Enumerate MADLAD-400 Somali JSONL.GZ shards.
///
/// Shard counts come from the upstream `madlad-400.py` builder script.
pub fn madlad_so_shards(include_noisy: bool) -> Vec<String> {
    let mut shards = vec![format!("data/{LANGUAGE}/{LANGUAGE}_clean_0000.jsonl.gz")];
    if include_noisy {
        shards.push(format!("data/{LANGUAGE}/{LANGUAGE}_noisy_0000.jsonl.gz"));
    }
    shards
}

pub fn dataset_repo() -> &'static str {
    DATASET_REPO
}

pub fn source_url(include_noisy: bool) -> String {
    let splits = if include_noisy {
        "clean+noisy"
    } else {
        "clean"
    };
    format!(
        "https://huggingface.co/datasets/{DATASET_REPO} (language: {LANGUAGE}, splits: {splits})"
    )
}
