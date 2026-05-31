pub mod resource;

pub use resource::{
    ActivityEntry, ActivityKind, ChangedFile, CheckCounts, CheckRun, CheckStatus, Commit,
    Deployment, MetadataItem, PullRequest, ReactionCounts, Resource, ResourceId, ResourceIdError,
    ResourceKind,
};
