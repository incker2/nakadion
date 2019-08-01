use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use crate::model::misc::{AuthorizationAttribute, OwningApplication};
pub use crate::model::EventTypeName;

/// Defines the category of this EventType.
///
/// The value set will influence, if not set otherwise, the default set of
/// validations, enrichment-strategies, and the effective schema for validation.
///
/// See also [Nakadi Manual](https://nakadi.io/manual.html#definition_EventType*category)
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum Category {
    /// No predefined changes apply. The effective schema for the validation is
    /// exactly the same as the EventTypeSchema.
    Undefined,
    /// Events of this category will be DataChangeEvents. The effective schema during
    /// the validation contains metadata, and adds fields data_op and data_type. The
    /// passed EventTypeSchema defines the schema of data.
    Data,
    /// Events of this category will be BusinessEvents. The effective schema for
    /// validation contains metadata and any additionally defined properties passed in the
    /// EventTypeSchema directly on top level of the Event. If name conflicts arise, creation
    /// of this EventType will be rejected.
    Business,
}

/// Determines how the assignment of the event to a partition should be handled.
///
/// The defualt is `random`.
///
/// See also [Nakadi Manual](https://nakadi.io/manual.html#/registry/partition-strategies_get)
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum PartitionStrategy {
    /// Resolution of the target partition happens randomly (events are evenly
    /// distributed on the topic’s partitions).
    Random,
    /// Resolution of the partition follows the computation of a hash from the value of
    /// the fields indicated in the EventType’s partition_key_fields, guaranteeing that Events
    /// with same values on those fields end in the same partition. Given the event type’s category
    /// is DataChangeEvent, field path is considered relative to “data”.
    Hash,
    /// Target partition is defined by the client. As long as the indicated
    /// partition exists, Event assignment will respect this value. Correctness of the relative
    /// ordering of events is under the responsibility of the Producer. Requires that the client
    /// provides the target partition on metadata.partition (See EventMetadata). Failure to do
    /// so will reject the publishing of the Event.
    UserDefined,
}

impl Default for PartitionStrategy {
    fn default() -> Self {
        PartitionStrategy::Random
    }
}

/// Compatibility mode provides a mean for event owners to evolve their schema, given changes respect the
/// semantics defined by this field.
///
/// It’s designed to be flexible enough so that producers can evolve their schemas while not
/// inadvertently breaking existent consumers.
///
/// Once defined, the compatibility mode is fixed, since otherwise it would break a predefined contract,
/// declared by the producer.
///
/// The default is `forward`.
///
/// See also [Nakadi Manual](https://nakadi.io/manual.html#definition_EventType*compatibility_mode)
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum CompatibilityMode {
    /// Consumers can reliably parse events produced under different versions. Every event published
    /// since the first version is still valid based on the newest schema. When in compatible mode, it’s allowed to
    /// add new optional properties and definitions to an existing schema, but no other changes are allowed.
    /// Under this mode, the following json-schema attributes are not supported: `not`, `patternProperties`,
    /// `additionalProperties` and `additionalItems`. When validating events, additional properties is `false`.
    Compatible,
    /// Compatible schema changes are allowed. It’s possible to use the full json schema specification
    /// for defining schemas. Consumers of forward compatible event types can safely read events tagged with the
    /// latest schema version as long as they follow the robustness principle.
    Forward,
    /// Any schema modification is accepted, even if it might break existing producers or consumers. When
    /// validating events, no additional properties are accepted unless explicitly stated in the schema.
    None,
}

impl Default for CompatibilityMode {
    fn default() -> Self {
        CompatibilityMode::Forward
    }
}

/// Part of `PartitionKeyFields`
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PartitionKey(String);

impl PartitionKey {
    pub fn new(v: impl Into<String>) -> Self {
        PartitionKey(v.into())
    }
}

/// Required when 'partition_resolution_strategy' is set to ‘hash’. Must be absent otherwise.
/// Indicates the fields used for evaluation the partition of Events of this type.
///
/// If this is set it MUST be a valid required field as defined in the schema.
///
/// See also [Nakadi Manual](https://nakadi.io/manual.html#definition_EventType*partition_key_fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionKeyFields(Vec<PartitionKey>);

/// Event type cleanup policy. There are two possible values.
///
/// It’s not possible to change the value of this field for existing event type.
///
/// See also [Nakadi Manual](https://nakadi.io/manual.html#definition_EventType*cleanup_policy)
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum CleanupPolicy {
    /// This cleanup policy will delete old events after retention time expires. Nakadi guarantees that each
    /// event will be available for at least the retention time period. However Nakadi doesn’t guarantee that event
    /// will be deleted right after retention time expires.
    Delete,
    /// This cleanup policy will keep only the latest event for each event key. The compaction is performed per
    /// partition, there is no compaction across partitions. The key that will be used as a compaction key should be
    /// specified in ‘partition_compaction_key’ field of event metadata. This cleanup policy is not available for
    /// ‘undefined’ category of event types.
    ///
    /// The compaction can be not applied to events that were published recently and located at the head of the
    /// queue, which means that the actual amount of events received by consumers can be different depending on time
    /// when the consumption happened.
    ///
    /// When using ‘compact’ cleanup policy user should consider that different Nakadi endpoints showing the amount
    /// of events will actually show the original amount of events published, not the actual amount of events that
    /// are currently there.
    /// E.g. subscription /stats endpoint will show the value ‘unconsumed_events’ - but that may not match with the
    /// actual amount of events unconsumed in that subscription as ‘compact’ cleanup policy may delete older events
    /// in the middle of queue if there is a newer event for the same key published.
    ///
    /// For more details about compaction implementation please read the documentation of Log Compaction in Kafka
    /// https://kafka.apache.org/documentation/#compaction, Nakadi currently relies on this implementation.
    Compact,
}

impl Default for CleanupPolicy {
    fn default() -> Self {
        CleanupPolicy::Delete
    }
}

/// The type of schema definition. Currently only json_schema (JSON Schema v04) is supported, but in the
/// future there could be others.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum SchemaType {
    #[serde(rename = "json_schema")]
    JsonSchema,
}

/// The most recent schema for this EventType. Submitted events will be validated against it.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventTypeSchema {
    version: String,
    created_at: DateTime<Utc>,
    #[serde(rename = "type")]
    schema_type: SchemaType,
    schema: String,
}

/// The most recent schema for this EventType. Submitted events will be validated against it.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventTypeSchemaInput {
    #[serde(rename = "type")]
    schema_type: SchemaType,
    schema: String,
}

/// Number of milliseconds that Nakadi stores events published to this event type.
///
/// See also [Nakadi Manual](https://nakadi.io/manual.html#definition_EventTypeOptions*retention_time)
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RetentionTime(Duration);

impl RetentionTime {
    pub fn new(d: Duration) -> Self {
        RetentionTime(d)
    }
}

impl Serialize for RetentionTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let millis = self.0.as_millis();
        serializer.serialize_u128(millis)
    }
}

impl<'de> Deserialize<'de> for RetentionTime {
    fn deserialize<D>(deserializer: D) -> Result<RetentionTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(RetentionTime(Duration::from_millis(millis)))
    }
}

/// Additional parameters for tuning internal behavior of Nakadi.
///
/// See also [Nakadi Manual](https://nakadi.io/manual.html#definition_EventTypeOptions)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EventTypeOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    retention_time: Option<RetentionTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTypeAuthorization {
    admins: Vec<AuthorizationAttribute>,
    readers: Vec<AuthorizationAttribute>,
    writers: Vec<AuthorizationAttribute>,
}

impl Default for EventTypeAuthorization {
    fn default() -> Self {
        Self {
            admins: Vec::default(),
            readers: Vec::default(),
            writers: Vec::default(),
        }
    }
}

/// Intended target audience of the event type. Relevant for standards around quality of design and documentation,
/// reviews, discoverability, changeability, and permission granting. See the guidelines
/// https://opensource.zalando.com/restful-api-guidelines/#219
///
/// This attribute adds no functionality and is used only to inform users about the usage scope of the event type.
///
/// See also [Nakadi Manual](https://nakadi.io/manual.html#definition_EventType*audience)
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum EventTypeAudience {
    ComponentInternal,
    BusinessUnitInternal,
    CompanyInternal,
    ExternalPartner,
    ExternalPublic,
}

/// Determines the enrichment to be performed on an Event upon reception. Enrichment is
/// performed once upon reception (and after validation) of an Event and is only possible on
/// fields that are not defined on the incoming Event.
///
/// For event types in categories `business` or `data` it’s mandatory to use
/// metadata_enrichment strategy. For `undefined` event types it’s not possible to use this
/// strategy, since metadata field is not required.
///
/// See documentation for the write operation for details on behaviour in case of unsuccessful
/// enrichment.
///
/// See also [Nakadi Manual](https://nakadi.io/manual.html#definition_EventType*enrichment_strategies)
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum EnrichmentStrategy {
    MetadataEnrichment,
}

/// Operational statistics for an EventType. This data may be provided by users on Event Type creation.
/// Nakadi uses this object in order to provide an optimal number of partitions from a throughput perspective.
///
/// See also [Nakadi Manual](https://nakadi.io/manual.html#definition_EventTypeStatistics)
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventTypeStatistics {
    /// Write rate for events of this EventType. This rate encompasses all producers of this
    /// EventType for a Nakadi cluster.
    ///
    /// Measured in event count per minute.
    messages_per_minute: u64,
    /// Average message size for each Event of this EventType. Includes in the count the whole serialized
    /// form of the event, including metadata.
    /// Measured in bytes.
    message_size: u64,
    /// Amount of parallel readers (consumers) to this EventType.
    read_parallelism: u64,
    /// Amount of parallel writers (producers) to this EventType.
    write_parallelism: u64,
}

/// Definition of an event type
///
/// This struct is only used for querying from Nakadi.
///
/// See also [Nakadi Manual](https://nakadi.io/manual.html#definition_EventType)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventType {
    name: EventTypeName,
    owning_application: OwningApplication,
    category: Category,
    enrichment_strategy: Option<EnrichmentStrategy>,
    partition_strategy: PartitionStrategy,
    compatibility_mode: CompatibilityMode,
    schema: EventTypeSchema,
    partition_key_fields: Option<PartitionKeyFields>,
    cleanup_policy: CleanupPolicy,
    default_statistic: Option<EventTypeStatistics>,
    options: Option<EventTypeOptions>,
    authorization: EventTypeAuthorization,
    audience: Option<EventTypeAudience>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Definition of an event type
///
/// This struct is only used for submitting data to Nakadi.
///
/// See also [Nakadi Manual](https://nakadi.io/manual.html#definition_EventType)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTypeInput {
    name: EventTypeName,
    owning_application: OwningApplication,
    category: Category,
    #[serde(skip_serializing_if = "Option::is_none")]
    enrichment_strategy: Option<EnrichmentStrategy>,
    partition_strategy: PartitionStrategy,
    compatibility_mode: CompatibilityMode,
    schema: EventTypeSchemaInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    partition_key_fields: Option<PartitionKeyFields>,
    cleanup_policy: CleanupPolicy,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_statistic: Option<EventTypeStatistics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<EventTypeOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authorization: Option<EventTypeAuthorization>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audience: Option<EventTypeAudience>,
}