//  Copyright 2023 Datafuse Labs.
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.

use std::collections::HashMap;

use chrono::DateTime;
use chrono::Utc;
use common_base::base::uuid::Uuid;
use common_exception::Result;
use common_expression::TableSchema;
use serde::Deserialize;
use serde::Serialize;

use crate::meta::format::compress;
use crate::meta::format::encode;
use crate::meta::format::Compression;
use crate::meta::monotonically_increased_timestamp;
use crate::meta::statistics::FormatVersion;
use crate::meta::trim_timestamp_to_micro_second;
use crate::meta::v2;
use crate::meta::ClusterKey;
use crate::meta::Encoding;
use crate::meta::Location;
use crate::meta::SnapshotId;
use crate::meta::Statistics;
use crate::meta::Versioned;

/// The structure of the segment is the same as that of v2, but the serialization and deserialization methods are different
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TableSnapshot {
    /// format version of snapshot
    format_version: FormatVersion,

    /// id of snapshot
    pub snapshot_id: SnapshotId,

    /// timestamp of this snapshot
    //  for backward compatibility, `Option` is used
    pub timestamp: Option<DateTime<Utc>>,

    /// previous snapshot
    pub prev_snapshot_id: Option<(SnapshotId, FormatVersion)>,

    /// For each snapshot, we keep a schema for it (in case of schema evolution)
    pub schema: TableSchema,

    /// Summary Statistics
    pub summary: Statistics,

    /// Pointers to SegmentInfos (may be of different format)
    ///
    /// We rely on background merge tasks to keep merging segments, so that
    /// this the size of this vector could be kept reasonable
    pub segments: Vec<Location>,

    // The metadata of the cluster keys.
    pub cluster_key_meta: Option<ClusterKey>,
    pub table_statistics_location: Option<String>,
}

impl TableSnapshot {
    pub fn new(
        snapshot_id: SnapshotId,
        prev_timestamp: &Option<DateTime<Utc>>,
        prev_snapshot_id: Option<(SnapshotId, FormatVersion)>,
        schema: TableSchema,
        summary: Statistics,
        segments: Vec<Location>,
        cluster_key_meta: Option<ClusterKey>,
        table_statistics_location: Option<String>,
    ) -> Self {
        let now = Utc::now();
        // make snapshot timestamp monotonically increased
        let adjusted_timestamp = monotonically_increased_timestamp(now, prev_timestamp);

        // trim timestamp to micro seconds
        let trimmed_timestamp = trim_timestamp_to_micro_second(adjusted_timestamp);
        let timestamp = Some(trimmed_timestamp);

        Self {
            format_version: TableSnapshot::VERSION,
            snapshot_id,
            timestamp,
            prev_snapshot_id,
            schema,
            summary,
            segments,
            cluster_key_meta,
            table_statistics_location,
        }
    }

    pub fn from_previous(previous: &TableSnapshot) -> Self {
        let id = Uuid::new_v4();
        let clone = previous.clone();
        // the timestamp of the new snapshot will be adjusted by the `new` method
        Self::new(
            id,
            &clone.timestamp,
            Some((clone.snapshot_id, clone.format_version)),
            clone.schema,
            clone.summary,
            clone.segments,
            clone.cluster_key_meta,
            clone.table_statistics_location,
        )
    }

    /// Serializes the struct to a byte vector.
    ///
    /// The byte vector contains the format version, encoding, compression, and compressed data. The encoding
    /// and compression are set to default values. The data is encoded and compressed.
    ///
    /// # Returns
    ///
    /// A Result containing the serialized data as a byte vector. If any errors occur during
    /// encoding, compression, or writing to the byte vector, an error will be returned.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let encoding = Encoding::default();
        let compression = Compression::default();

        let data = encode(&encoding, &self)?;
        let data_compress = compress(&compression, data)?;

        let data_size = self.format_version.to_le_bytes().len()
            + 2
            + data_compress.len().to_le_bytes().len()
            + data_compress.len();
        let mut buf = Vec::with_capacity(data_size);

        buf.extend_from_slice(&self.format_version.to_le_bytes());
        buf.push(encoding as u8);
        buf.push(compression as u8);
        buf.extend_from_slice(&data_compress.len().to_le_bytes());

        buf.extend(data_compress);

        Ok(buf)
    }

    #[inline]
    pub fn encoding() -> Encoding {
        Encoding::default()
    }

    pub fn format_version(&self) -> u64 {
        self.format_version
    }

    pub fn build_segment_id_map(&self) -> HashMap<String, usize> {
        let segment_count = self.segments.len();
        let mut segment_id_map = HashMap::new();
        for (i, segment_loc) in self.segments.iter().enumerate() {
            segment_id_map.insert(segment_loc.0.to_string(), segment_count - i - 1);
        }
        segment_id_map
    }
}

impl From<v2::TableSnapshot> for TableSnapshot {
    fn from(s: v2::TableSnapshot) -> Self {
        Self {
            format_version: TableSnapshot::VERSION,
            snapshot_id: s.snapshot_id,
            timestamp: s.timestamp,
            prev_snapshot_id: s.prev_snapshot_id,
            schema: s.schema,
            summary: s.summary,
            segments: s.segments,
            cluster_key_meta: s.cluster_key_meta,
            table_statistics_location: s.table_statistics_location,
        }
    }
}

// A memory light version of TableSnapshot(Without segments)
// This *ONLY* used for some optimize operation, like PURGE/FUSE_SNAPSHOT function to avoid OOM.
#[derive(Clone, Debug)]
pub struct TableSnapshotLite {
    pub format_version: FormatVersion,
    pub snapshot_id: SnapshotId,
    pub timestamp: Option<DateTime<Utc>>,
    pub prev_snapshot_id: Option<(SnapshotId, FormatVersion)>,
    pub row_count: u64,
    pub block_count: u64,
    pub index_size: u64,
    pub uncompressed_byte_size: u64,
    pub compressed_byte_size: u64,
    pub segment_count: u64,
}

impl From<(&TableSnapshot, FormatVersion)> for TableSnapshotLite {
    fn from((value, ver): (&TableSnapshot, FormatVersion)) -> Self {
        TableSnapshotLite {
            format_version: ver,
            snapshot_id: value.snapshot_id,
            timestamp: value.timestamp,
            prev_snapshot_id: value.prev_snapshot_id,
            row_count: value.summary.row_count,
            block_count: value.summary.block_count,
            index_size: value.summary.index_size,
            uncompressed_byte_size: value.summary.uncompressed_byte_size,
            segment_count: value.segments.len() as u64,
            compressed_byte_size: value.summary.compressed_byte_size,
        }
    }
}