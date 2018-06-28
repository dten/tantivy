use super::SegmentComponent;
use core::SegmentId;
use std::collections::HashSet;
use std::path::PathBuf;
use census::{TrackedObject, Inventory};
use std::fmt;
use serde;

lazy_static! {
    static ref INVENTORY: Inventory<InnerSegmentMeta>  = {
        Inventory::new()
    };
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DeleteMeta {
    num_deleted_docs: u32,
    opstamp: u64,
}

/// `SegmentMeta` contains simple meta information about a segment.
///
/// For instance the number of docs it contains,
/// how many are deleted, etc.
#[derive(Clone)]
pub struct SegmentMeta {
    inner: TrackedObject<InnerSegmentMeta>,
}

impl fmt::Debug for SegmentMeta {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.inner.fmt(f)
    }
}

impl serde::Serialize for SegmentMeta {
    fn serialize<S>(&self, serializer: S) -> Result<<S as serde::Serializer>::Ok, <S as serde::Serializer>::Error> where
        S: serde::Serializer {
        self.inner.serialize(serializer)
    }
}

impl<'a> serde::Deserialize<'a> for SegmentMeta {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as serde::Deserializer<'a>>::Error> where
        D: serde::Deserializer<'a> {
        let inner = InnerSegmentMeta::deserialize(deserializer)?;
        let tracked = INVENTORY.track(inner);
        Ok(SegmentMeta { inner: tracked })
    }
}

impl SegmentMeta {

    /// Returns a snapshot of all living `SegmentMeta` object.
    pub fn all() -> Vec<SegmentMeta> {
        INVENTORY.list().into_iter().map(|inner| SegmentMeta {inner}).collect::<Vec<_>>()
    }

    /// Creates a new segment meta for
    /// a segment with no deletes and no documents.
    pub fn new(segment_id: SegmentId) -> SegmentMeta {
        let inner = InnerSegmentMeta::new(segment_id);
        let tracked = INVENTORY.track(inner);
        SegmentMeta {
            inner: tracked,
        }
    }

    /// Returns the segment id.
    pub fn id(&self) -> SegmentId {
        self.inner.segment_id
    }

    /// Returns the number of deleted documents.
    pub fn num_deleted_docs(&self) -> u32 {
        self.inner
            .deletes
            .as_ref()
            .map(|delete_meta| delete_meta.num_deleted_docs)
            .unwrap_or(0u32)
    }

    /// Returns the list of files that
    /// are required for the segment meta.
    ///
    /// This is useful as the way tantivy removes files
    /// is by removing all files that have been created by tantivy
    /// and are not used by any segment anymore.
    pub fn list_files(&self) -> HashSet<PathBuf> {
        SegmentComponent::iterator()
            .map(|component| self.relative_path(*component))
            .collect::<HashSet<PathBuf>>()
    }

    /// Returns the relative path of a component of our segment.
    ///
    /// It just joins the segment id with the extension
    /// associated to a segment component.
    pub fn relative_path(&self, component: SegmentComponent) -> PathBuf {
        let mut path = self.id().uuid_string();
        path.push_str(&*match component {
            SegmentComponent::POSITIONS => ".pos".to_string(),
            SegmentComponent::POSTINGS => ".idx".to_string(),
            SegmentComponent::TERMS => ".term".to_string(),
            SegmentComponent::STORE => ".store".to_string(),
            SegmentComponent::FASTFIELDS => ".fast".to_string(),
            SegmentComponent::FIELDNORMS => ".fieldnorm".to_string(),
            SegmentComponent::DELETE => format!(".{}.del", self.delete_opstamp().unwrap_or(0)),
        });
        PathBuf::from(path)
    }

    /// Return the highest doc id + 1
    ///
    /// If there are no deletes, then num_docs = max_docs
    /// and all the doc ids contains in this segment
    /// are exactly (0..max_doc).
    pub fn max_doc(&self) -> u32 {
        self.inner.max_doc
    }

    /// Return the number of documents in the segment.
    pub fn num_docs(&self) -> u32 {
        self.max_doc() - self.num_deleted_docs()
    }

    /// Returns the opstamp of the last delete operation
    /// taken in account in this segment.
    pub fn delete_opstamp(&self) -> Option<u64> {
        self.inner
            .deletes
            .as_ref()
            .map(|delete_meta| delete_meta.opstamp)
    }

    /// Returns true iff the segment meta contains
    /// delete information.
    pub fn has_deletes(&self) -> bool {
        self.num_deleted_docs() > 0
    }

    #[doc(hidden)]
    pub fn with_max_doc(self, max_doc: u32) -> SegmentMeta {
        let tracked = self.inner
            .map(move |inner_meta| {
                let inner_meta_clone = inner_meta.clone();
                InnerSegmentMeta {
                    segment_id: inner_meta_clone.segment_id,
                    max_doc,
                    deletes: inner_meta_clone.deletes,
                }
            });
        SegmentMeta {
            inner: tracked
        }
    }

    #[doc(hidden)]
    pub fn with_delete_meta(self, num_deleted_docs: u32, opstamp: u64) -> SegmentMeta {
        let delete_meta = DeleteMeta {
            num_deleted_docs,
            opstamp,
        };
        let tracked = self.inner
            .map(move |inner_meta| {
                InnerSegmentMeta {
                    segment_id: inner_meta.segment_id,
                    max_doc: inner_meta.max_doc,
                    deletes: Some(delete_meta),
                }
            });
        SegmentMeta {
            inner: tracked
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InnerSegmentMeta {
    segment_id: SegmentId,
    max_doc: u32,
    deletes: Option<DeleteMeta>,
}

impl InnerSegmentMeta {
    pub fn new(segment_id: SegmentId) -> InnerSegmentMeta {
        InnerSegmentMeta {
            segment_id,
            max_doc: 0,
            deletes: None,
        }
    }
}
