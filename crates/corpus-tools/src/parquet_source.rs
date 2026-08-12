use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result};
use arrow_array::cast::AsArray;
use arrow_array::{Array, RecordBatch, StructArray};
use flate2::read::GzDecoder;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

pub fn iter_text_column(path: &Path, column: &str) -> Result<impl Iterator<Item = Result<String>>> {
    let column = column.to_string();
    let file = File::open(path).with_context(|| format!("opening parquet file {}", path.display()))?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)?
        .build()
        .context("building parquet reader")?;

    Ok(reader.flat_map(move |batch| {
        let batch = match batch {
            Ok(batch) => batch,
            Err(error) => return vec![Err(error.into())],
        };
        extract_string_column(&batch, &column)
    }))
}

/// Read several string columns at once, yielding one `Vec<String>` per row in
/// the order `columns` was given.
///
/// Needed when a record keeps provenance alongside its text (XL-Sum carries
/// `id` / `url` next to `title` / `text` / `summary`); [`iter_text_column`]
/// would require one pass per column and could not zip the passes back into
/// rows safely.
pub fn iter_string_rows(
    path: &Path,
    columns: &[String],
) -> Result<impl Iterator<Item = Result<Vec<String>>>> {
    let columns: Vec<String> = columns.to_vec();
    let file = File::open(path).with_context(|| format!("opening parquet file {}", path.display()))?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)?
        .build()
        .context("building parquet reader")?;

    Ok(reader.flat_map(move |batch| {
        let batch = match batch {
            Ok(batch) => batch,
            Err(error) => return vec![Err(error.into())],
        };
        extract_string_rows(&batch, &columns)
    }))
}

/// Row count recorded in a parquet file's footer (no row scan required).
pub fn row_count(path: &Path) -> Result<u64> {
    let file = File::open(path).with_context(|| format!("opening parquet file {}", path.display()))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    let rows = builder.metadata().file_metadata().num_rows();
    Ok(rows.max(0) as u64)
}

/// Column names paired with their arrow data type, read from the footer.
pub fn column_types(path: &Path) -> Result<Vec<(String, String)>> {
    let file = File::open(path).with_context(|| format!("opening parquet file {}", path.display()))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    Ok(builder
        .schema()
        .fields()
        .iter()
        .map(|field| (field.name().clone(), format!("{:?}", field.data_type())))
        .collect())
}

pub fn iter_struct_field(
    path: &Path,
    column: &str,
    field: &str,
) -> Result<impl Iterator<Item = Result<String>>> {
    let column = column.to_string();
    let field = field.to_string();
    let file = File::open(path).with_context(|| format!("opening parquet file {}", path.display()))?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)?
        .build()
        .context("building parquet reader")?;

    Ok(reader.flat_map(move |batch| {
        let batch = match batch {
            Ok(batch) => batch,
            Err(error) => return vec![Err(error.into())],
        };
        extract_struct_string_field(&batch, &column, &field)
    }))
}

fn extract_string_column(batch: &RecordBatch, column: &str) -> Vec<Result<String>> {
    let schema = batch.schema();
    let index = match schema.index_of(column) {
        Ok(index) => index,
        Err(error) => return vec![Err(error.into())],
    };

    let array = batch.column(index);
    string_values(array, column)
}

fn extract_string_rows(batch: &RecordBatch, columns: &[String]) -> Vec<Result<Vec<String>>> {
    let mut per_column = Vec::with_capacity(columns.len());
    for column in columns {
        let mut values = Vec::with_capacity(batch.num_rows());
        for value in extract_string_column(batch, column) {
            match value {
                Ok(value) => values.push(value),
                Err(error) => return vec![Err(error)],
            }
        }
        per_column.push(values);
    }

    (0..batch.num_rows())
        .map(|row| {
            Ok(per_column
                .iter()
                .map(|values| values[row].clone())
                .collect::<Vec<String>>())
        })
        .collect()
}

fn extract_struct_string_field(batch: &RecordBatch, column: &str, field: &str) -> Vec<Result<String>> {
    let schema = batch.schema();
    let index = match schema.index_of(column) {
        Ok(index) => index,
        Err(error) => return vec![Err(error.into())],
    };

    let array = batch.column(index);
    let Some(struct_array) = array.as_any().downcast_ref::<StructArray>() else {
        return vec![Err(anyhow::anyhow!("column {column} is not a struct"))];
    };

    let field_index = match struct_array.column_names().iter().position(|name| *name == field) {
        Some(index) => index,
        None => {
            return vec![Err(anyhow::anyhow!(
                "field {field} not found in struct column {column}"
            ))];
        }
    };

    string_values(struct_array.column(field_index), &format!("{column}.{field}"))
}

fn string_values(array: &dyn Array, label: &str) -> Vec<Result<String>> {
    match array.data_type() {
        arrow_schema::DataType::Utf8 => {
            let strings = array.as_string::<i32>();
            (0..strings.len())
                .map(|idx| {
                    if strings.is_null(idx) {
                        Ok(String::new())
                    } else {
                        Ok(strings.value(idx).to_string())
                    }
                })
                .collect()
        }
        arrow_schema::DataType::LargeUtf8 => {
            let strings = array.as_string::<i64>();
            (0..strings.len())
                .map(|idx| {
                    if strings.is_null(idx) {
                        Ok(String::new())
                    } else {
                        Ok(strings.value(idx).to_string())
                    }
                })
                .collect()
        }
        other => vec![Err(anyhow::anyhow!(
            "unsupported string column type for {label}: {other:?}"
        ))],
    }
}

pub fn iter_json_gz_text(path: &Path) -> Result<impl Iterator<Item = Result<String>>> {
    let file = File::open(path).with_context(|| format!("opening gzip file {}", path.display()))?;
    let decoder = GzDecoder::new(file);
    let reader = BufReader::new(decoder);

    Ok(reader.lines().map(|line| {
        let line = line.context("reading gzip json line")?;
        let value: serde_json::Value = serde_json::from_str(&line).context("parsing json line")?;
        Ok(value
            .get("text")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string())
    }))
}
