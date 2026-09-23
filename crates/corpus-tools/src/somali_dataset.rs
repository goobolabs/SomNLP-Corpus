//! Somali Alpaca instruction dataset (`burtugeey/Somali_dataset`) downloader.
//!
//! Each row is an Alpaca-style `instruction` / `input` / `output` triple. Parquet
//! representations are available on [`PARQUET_REVISION`], under `default/train/`.
//! Every row becomes one document: its non-empty fields joined, in that order, by a
//! blank line, with no synthetic labels added.

use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use tempfile::NamedTempFile;

use crate::hf::{HfClient, PARQUET_REVISION};
use crate::jsonl::{is_non_empty, JsonlWriter};
use crate::parquet_source::iter_text_column;

pub const SOURCE_TAG: &str = "somali_dataset";
pub const DATASET_NAME: &str = "burtugeey/Somali_dataset";
pub const DATASET_PREFIX: &str = "default/train";
pub const INSTRUCTION_COLUMN: &str = "instruction";
pub const INPUT_COLUMN: &str = "input";
pub const OUTPUT_COLUMN: &str = "output";
/// Misspelled column that holds the prompt on the upstream rows whose `instruction` is null.
pub const LEGACY_INSTRUCTION_COLUMN: &str = "instructions";
/// Separator placed between the included fields of one document.
pub const FIELD_SEPARATOR: &str = "\n\n";

/// Human-readable provenance line for the export summary.
pub fn source_url() -> String {
    format!("https://huggingface.co/datasets/{DATASET_NAME} (revision: {PARQUET_REVISION})")
}

/// Download the Somali Alpaca parquet shards and export one JSONL record per row with usable text.
pub fn download_somali_dataset(
    output: &Path,
    limit: Option<u64>,
    streaming: bool,
) -> Result<crate::Stats> {
    let hf = HfClient::new();
    let listed = hf.list_files_at(DATASET_NAME, PARQUET_REVISION, DATASET_PREFIX, true)?;
    let shards = select_shards(&listed)?;

    let mut writer = JsonlWriter::create(output, "Writing")?;
    let mut written = 0u64;
    let mut temps: Vec<NamedTempFile> = Vec::new();

    for remote_path in &shards {
        if limit.is_some_and(|limit| written >= limit) {
            break;
        }

        let local_path = resolve_shard(&hf, remote_path, output, streaming, &mut temps)?;

        for row in iter_rows(&local_path)? {
            if limit.is_some_and(|limit| written >= limit) {
                break;
            }
            let row = row?;
            let Some(text) = format_document(&row.instruction, &row.input, &row.output) else {
                continue;
            };
            writer.write_text(&text)?;
            written += 1;
        }
    }

    let stats = writer.stats.clone();
    writer.finish();

    if stats.total_docs == 0 {
        bail!("no documents exported from {DATASET_NAME}");
    }

    crate::cli::print_export_summary(
        "Somali Alpaca dataset export complete",
        &stats,
        output,
        &source_url(),
    );
    Ok(stats)
}

/// One source row, with the prompt already resolved from `instruction` or its legacy column.
#[derive(Debug, PartialEq)]
struct Row {
    instruction: String,
    input: String,
    output: String,
}

/// Iterate the rows of a shard. `instruction`, `input`, and `output` are required;
/// [`LEGACY_INSTRUCTION_COLUMN`] fills in the prompt only when `instruction` is empty.
fn iter_rows(path: &Path) -> Result<impl Iterator<Item = Result<Row>>> {
    let instructions = iter_text_column(path, INSTRUCTION_COLUMN)?;
    let inputs = iter_text_column(path, INPUT_COLUMN)?;
    let outputs = iter_text_column(path, OUTPUT_COLUMN)?;
    let legacy: Box<dyn Iterator<Item = Result<String>>> =
        if has_column(path, LEGACY_INSTRUCTION_COLUMN)? {
            Box::new(iter_text_column(path, LEGACY_INSTRUCTION_COLUMN)?)
        } else {
            Box::new(std::iter::repeat_with(|| Ok(String::new())))
        };

    Ok(instructions.zip(inputs).zip(outputs).zip(legacy).map(
        |(((instruction, input), output), legacy)| {
            let instruction = instruction?;
            let legacy = legacy?;
            Ok(Row {
                instruction: if is_non_empty(&instruction) {
                    instruction
                } else {
                    legacy
                },
                input: input?,
                output: output?,
            })
        },
    ))
}

fn has_column(path: &Path, column: &str) -> Result<bool> {
    let file =
        File::open(path).with_context(|| format!("opening parquet file {}", path.display()))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    Ok(builder.schema().column_with_name(column).is_some())
}

/// Join the non-empty fields in `instruction`, `input`, `output` order, or `None` when all are empty.
fn format_document(instruction: &str, input: &str, output: &str) -> Option<String> {
    let fields: Vec<&str> = [instruction, input, output]
        .into_iter()
        .filter(|field| is_non_empty(field))
        .collect();
    if fields.is_empty() {
        return None;
    }
    Some(fields.join(FIELD_SEPARATOR))
}

/// Pick the parquet shards belonging to `default/train/`, sorted deterministically.
fn select_shards(listed: &[String]) -> Result<Vec<String>> {
    let prefix = format!("{DATASET_PREFIX}/");
    let mut matched: Vec<String> = listed
        .iter()
        .filter(|path| path.starts_with(&prefix) && path.ends_with(".parquet"))
        .cloned()
        .collect();

    if matched.is_empty() {
        bail!("no parquet shards found under {DATASET_PREFIX}/");
    }

    matched.sort();
    Ok(matched)
}

fn resolve_shard(
    hf: &HfClient,
    remote_path: &str,
    output: &Path,
    streaming: bool,
    temps: &mut Vec<NamedTempFile>,
) -> Result<PathBuf> {
    if streaming {
        let (temp, path) = hf.download_to_temp_at(DATASET_NAME, PARQUET_REVISION, remote_path)?;
        temps.push(temp);
        return Ok(path);
    }

    let path = output
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(local_shard_name(remote_path));
    hf.download_to_path_at(DATASET_NAME, PARQUET_REVISION, remote_path, &path)?;
    Ok(path)
}

/// Flatten a remote shard path into a unique local filename.
fn local_shard_name(remote_path: &str) -> String {
    let flattened = remote_path.trim_start_matches('/').replace('/', "_");
    format!("{SOURCE_TAG}_{flattened}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{ArrayRef, RecordBatch, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use parquet::arrow::arrow_writer::ArrowWriter;
    use std::sync::Arc;

    fn listing() -> Vec<String> {
        vec![
            "default/train/0001.parquet".to_string(),
            "default/train/0000.parquet".to_string(),
            "default/test/0000.parquet".to_string(),
            "default/train_extra/0000.parquet".to_string(),
            "default/train/not_parquet.txt".to_string(),
            ".gitattributes".to_string(),
        ]
    }

    fn string_field(name: &str) -> Field {
        Field::new(name, DataType::Utf8, true)
    }

    fn string_column(values: Vec<Option<&str>>) -> ArrayRef {
        Arc::new(StringArray::from(values))
    }

    fn create_test_parquet(fields: Vec<Field>, columns: Vec<ArrayRef>) -> (NamedTempFile, PathBuf) {
        let schema = Arc::new(Schema::new(fields));
        let batch = RecordBatch::try_new(schema.clone(), columns).unwrap();

        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();
        let file = File::create(&path).unwrap();
        let mut writer = ArrowWriter::try_new(file, schema, None).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();
        (temp, path)
    }

    fn documents(path: &Path) -> Vec<String> {
        iter_rows(path)
            .unwrap()
            .map(|row| row.unwrap())
            .filter_map(|row| format_document(&row.instruction, &row.input, &row.output))
            .collect()
    }

    #[test]
    fn filters_to_exact_default_train_prefix() {
        let shards = select_shards(&listing()).unwrap();
        assert!(shards.iter().all(|s| s.starts_with("default/train/")));
        assert!(shards.iter().all(|s| s.ends_with(".parquet")));
        assert!(!shards.iter().any(|s| s.contains("test")));
        assert!(!shards.iter().any(|s| s.contains("train_extra")));
    }

    #[test]
    fn orders_shards_deterministically() {
        let shards = select_shards(&listing()).unwrap();
        assert_eq!(
            shards,
            vec![
                "default/train/0000.parquet".to_string(),
                "default/train/0001.parquet".to_string(),
            ]
        );
    }

    #[test]
    fn missing_or_empty_shards_return_error() {
        let empty: Vec<String> = vec![];
        assert!(select_shards(&empty).is_err());

        let non_matching = vec![
            "default/test/0000.parquet".to_string(),
            ".gitattributes".to_string(),
        ];
        assert!(select_shards(&non_matching).is_err());
    }

    #[test]
    fn local_shard_names_are_collision_safe() {
        assert_eq!(
            local_shard_name("default/train/0000.parquet"),
            "somali_dataset_default_train_0000.parquet"
        );
        assert_ne!(
            local_shard_name("default/train/0000.parquet"),
            local_shard_name("default/train/0001.parquet")
        );
    }

    #[test]
    fn formats_instruction_and_output_when_input_is_empty() {
        assert_eq!(
            format_document("Waa maxay caasimadda?", "", "Muqdisho."),
            Some("Waa maxay caasimadda?\n\nMuqdisho.".to_string())
        );
        assert_eq!(
            format_document("Waa maxay caasimadda?", "  \n", "Muqdisho."),
            Some("Waa maxay caasimadda?\n\nMuqdisho.".to_string())
        );
    }

    #[test]
    fn formats_all_three_fields_in_order() {
        assert_eq!(
            format_document("Turjun.", "Good morning", "Subax wanaagsan"),
            Some("Turjun.\n\nGood morning\n\nSubax wanaagsan".to_string())
        );
    }

    #[test]
    fn adds_no_synthetic_labels() {
        let text = format_document("Su'aal", "Gelin", "Jawaab").unwrap();
        assert!(!text.contains("Instruction:"));
        assert!(!text.contains("Input:"));
        assert!(!text.contains("Response:"));
    }

    #[test]
    fn skips_rows_without_usable_text() {
        assert_eq!(format_document("", "", ""), None);
        assert_eq!(format_document(" ", "\n", "\t"), None);
    }

    #[test]
    fn reads_rows_and_falls_back_to_legacy_instruction_column() {
        let fields = vec![
            string_field(INSTRUCTION_COLUMN),
            string_field(OUTPUT_COLUMN),
            string_field(INPUT_COLUMN),
            string_field(LEGACY_INSTRUCTION_COLUMN),
        ];
        let columns = vec![
            string_column(vec![
                Some("Su'aal 1"),
                None,
                Some(""),
                None,
                Some("Su'aal 4"),
            ]),
            string_column(vec![
                Some("Jawaab 1"),
                Some("Jawaab 2"),
                Some(""),
                None,
                Some("J4"),
            ]),
            string_column(vec![Some(""), Some(""), Some(""), None, Some("Gelin 4")]),
            string_column(vec![None, Some("Su'aal 2"), None, None, Some("ignored")]),
        ];
        let (_temp, path) = create_test_parquet(fields, columns);

        assert_eq!(
            documents(&path),
            vec![
                "Su'aal 1\n\nJawaab 1".to_string(),
                "Su'aal 2\n\nJawaab 2".to_string(),
                "Su'aal 4\n\nGelin 4\n\nJ4".to_string(),
            ]
        );
    }

    #[test]
    fn legacy_instruction_column_is_optional() {
        let fields = vec![
            string_field(INSTRUCTION_COLUMN),
            string_field(INPUT_COLUMN),
            string_field(OUTPUT_COLUMN),
        ];
        let columns = vec![
            string_column(vec![Some("Su'aal")]),
            string_column(vec![Some("")]),
            string_column(vec![Some("Jawaab")]),
        ];
        let (_temp, path) = create_test_parquet(fields, columns);

        assert_eq!(documents(&path), vec!["Su'aal\n\nJawaab".to_string()]);
    }

    #[test]
    fn missing_required_column_returns_error() {
        let fields = vec![string_field(INSTRUCTION_COLUMN), string_field(INPUT_COLUMN)];
        let columns = vec![
            string_column(vec![Some("Su'aal")]),
            string_column(vec![Some("")]),
        ];
        let (_temp, path) = create_test_parquet(fields, columns);

        let rows: Result<Vec<Row>> = iter_rows(&path).and_then(|rows| rows.collect());
        assert!(rows.is_err());
    }
}
