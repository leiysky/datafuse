// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fmt;
use std::str::FromStr;

use chrono::DateTime;
use chrono::Utc;
use common_storage::StorageParams;

use crate::UserIdentity;

// -- Internal stage
// CREATE [ OR REPLACE ] [ TEMPORARY ] STAGE [ IF NOT EXISTS ] <internal_stage_name>
// internalStageParams
// directoryTableParams
// [ FILE_FORMAT = ( { FORMAT_NAME = '<file_format_name>' | TYPE = { CSV | JSON | AVRO | ORC | PARQUET | XML } [ formatTypeOptions ] ) } ]
// [ COPY_OPTIONS = ( copyOptions ) ]
// [ COMMENT = '<string_literal>' ]
//
// -- External stage
// CREATE [ OR REPLACE ] [ TEMPORARY ] STAGE [ IF NOT EXISTS ] <external_stage_name>
// externalStageParams
// directoryTableParams
// [ FILE_FORMAT = ( { FORMAT_NAME = '<file_format_name>' | TYPE = { CSV | JSON | AVRO | ORC | PARQUET | XML } [ formatTypeOptions ] ) } ]
// [ COPY_OPTIONS = ( copyOptions ) ]
// [ COMMENT = '<string_literal>' ]
//
//
// WHERE
//
// externalStageParams (for Amazon S3) ::=
// URL = 's3://<bucket>[/<path>/]'
// [ { CREDENTIALS = ( {  { AWS_KEY_ID = '<string>' AWS_SECRET_KEY = '<string>' [ AWS_TOKEN = '<string>' ] } | AWS_ROLE = '<string>'  } ) ) } ]
//
// copyOptions ::=
// ON_ERROR = { CONTINUE | SKIP_FILE | SKIP_FILE_<num> | SKIP_FILE_<num>% | ABORT_STATEMENT }
// SIZE_LIMIT = <num>

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum StageType {
    /// LegacyInternal will be depracated.
    ///
    /// Please never use this variant except in `proto_conv`. We keep this
    /// stage type for backword compatible.
    ///
    /// TODO(xuanwo): remove this when we are releasing v0.9.
    LegacyInternal,
    External,
    Internal,
    /// User Stage is the stage for every sql user.
    ///
    /// This is a stage that just in memory. We will not persist in metasrv
    User,
}

impl fmt::Display for StageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            // LegacyInternal will print the same name as Internal, this is by design.
            StageType::LegacyInternal => "Internal",
            StageType::External => "External",
            StageType::Internal => "Internal",
            StageType::User => "User",
        };
        write!(f, "{}", name)
    }
}

impl Default for StageType {
    fn default() -> Self {
        Self::External
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageFileCompression {
    Auto,
    Gzip,
    Bz2,
    Brotli,
    Zstd,
    Deflate,
    RawDeflate,
    Lzo,
    Snappy,
    Xz,
    None,
}

impl Default for StageFileCompression {
    fn default() -> Self {
        Self::None
    }
}

impl FromStr for StageFileCompression {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(StageFileCompression::Auto),
            "gzip" => Ok(StageFileCompression::Gzip),
            "bz2" => Ok(StageFileCompression::Bz2),
            "brotli" => Ok(StageFileCompression::Brotli),
            "zstd" => Ok(StageFileCompression::Zstd),
            "deflate" => Ok(StageFileCompression::Deflate),
            "rawdeflate" | "raw_deflate" => Ok(StageFileCompression::RawDeflate),
            "lzo" => Ok(StageFileCompression::Lzo),
            "snappy" => Ok(StageFileCompression::Snappy),
            "xz" => Ok(StageFileCompression::Xz),
            "none" => Ok(StageFileCompression::None),
            _ => Err("Unknown file compression type, must one of { auto | gzip | bz2 | brotli | zstd | deflate | raw_deflate | lzo | snappy | xz | none }"
                         .to_string()),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum StageFileFormatType {
    Csv,
    Tsv,
    Json,
    NdJson,
    Avro,
    Orc,
    Parquet,
    Xml,
}

impl Default for StageFileFormatType {
    fn default() -> Self {
        Self::Csv
    }
}

impl FromStr for StageFileFormatType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s.to_uppercase().as_str() {
            "CSV" => Ok(StageFileFormatType::Csv),
            "TSV" | "TABSEPARATED" => Ok(StageFileFormatType::Tsv),
            "NDJSON" | "JSONEACHROW" => Ok(StageFileFormatType::NdJson),
            "PARQUET" => Ok(StageFileFormatType::Parquet),
            "XML" => Ok(StageFileFormatType::Xml),
            "ORC" | "AVRO" | "JSON" => Err(format!(
                "File format type '{s}' not implemented yet', must be one of ( CSV | TSV | NDJSON | PARQUET | XML)"
            )),
            _ => Err(format!(
                "Unknown file format type '{s}', must be one of ( CSV | TSV | NDJSON | PARQUET | XML)"
            )),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(default)]
pub struct FileFormatOptions {
    pub format: StageFileFormatType,
    // Number of lines at the start of the file to skip.
    pub skip_header: u64,
    pub field_delimiter: String,
    pub record_delimiter: String,
    pub escape: String,
    pub compression: StageFileCompression,
    pub row_tag: String,
}

impl Default for FileFormatOptions {
    fn default() -> Self {
        Self {
            format: StageFileFormatType::default(),
            record_delimiter: "\n".to_string(),
            field_delimiter: ",".to_string(),
            skip_header: 0,
            escape: "".to_string(),
            compression: StageFileCompression::default(),
            row_tag: "row".to_string(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug, Eq, PartialEq)]
#[serde(default)]
pub struct StageParams {
    pub storage: StorageParams,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum OnErrorMode {
    None,
    Continue,
    SkipFile,
    SkipFileNum(u64),
    AbortStatement,
}

impl Default for OnErrorMode {
    fn default() -> Self {
        Self::None
    }
}

impl FromStr for OnErrorMode {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s.to_uppercase().as_str() {
            "" => Ok(OnErrorMode::None),
            "CONTINUE" => Ok(OnErrorMode::Continue),
            "SKIP_FILE" => Ok(OnErrorMode::SkipFile),
            v => {
                let num_str = v.replace("SKIP_FILE_", "");
                let nums = num_str.parse::<u64>();
                match nums {
                    Ok(v) => Ok(OnErrorMode::SkipFileNum(v)),
                    Err(_) => Err(format!(
                        "Unknown OnError mode:{:?}, must one of {{ CONTINUE | SKIP_FILE | SKIP_FILE_<num> | ABORT_STATEMENT }}",
                        v
                    )),
                }
            }
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default, Debug, Eq, PartialEq)]
#[serde(default)]
pub struct CopyOptions {
    pub on_error: OnErrorMode,
    pub size_limit: usize,
    pub split_size: usize,
    pub purge: bool,
    pub single: bool,
    pub max_file_size: usize,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug, Eq, PartialEq)]
#[serde(default)]
pub struct UserStageInfo {
    pub stage_name: String,
    pub stage_type: StageType,
    pub stage_params: StageParams,
    pub file_format_options: FileFormatOptions,
    pub copy_options: CopyOptions,
    pub comment: String,
    /// TODO(xuanwo): stage doesn't have this info anymore, remove it.
    pub number_of_files: u64,
    pub creator: Option<UserIdentity>,
}

impl UserStageInfo {
    /// Create a new internal stage.
    pub fn new_internal_stage(name: &str) -> UserStageInfo {
        UserStageInfo {
            stage_name: name.to_string(),
            stage_type: StageType::Internal,
            ..Default::default()
        }
    }

    pub fn new_external_stage(storage: StorageParams, path: &str) -> UserStageInfo {
        UserStageInfo {
            stage_name: format!("{storage},path={path}"),
            stage_type: StageType::External,
            stage_params: StageParams { storage },
            ..Default::default()
        }
    }

    /// Create a new user stage.
    pub fn new_user_stage(user: &str) -> UserStageInfo {
        UserStageInfo {
            stage_name: user.to_string(),
            stage_type: StageType::User,
            ..Default::default()
        }
    }

    /// Update user stage with stage name.
    pub fn with_stage_name(mut self, name: &str) -> UserStageInfo {
        self.stage_name = name.to_string();
        self
    }

    /// Get the prefix of stage.
    ///
    /// Use this function to get the prefix of this stage in the data operator.
    ///
    /// # Notes
    ///
    /// This function should never be called on external stage because it's meanless. Something must be wrong.
    pub fn stage_prefix(&self) -> String {
        match self.stage_type {
            StageType::LegacyInternal => format!("/stage/{}/", self.stage_name),
            StageType::External => {
                unreachable!("stage_prefix should never be called on external stage, must be a bug")
            }
            StageType::Internal => format!("/stage/internal/{}/", self.stage_name),
            StageType::User => format!("/stage/user/{}/", self.stage_name),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct StageFile {
    pub path: String,
    pub size: u64,
    pub md5: Option<String>,
    pub last_modified: DateTime<Utc>,
    pub creator: Option<UserIdentity>,
    pub etag: Option<String>,
}
