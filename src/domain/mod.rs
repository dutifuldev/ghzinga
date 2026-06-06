pub mod resource;

pub use resource::{
    ActivityEntry, ActivityKind, ChangedFile, CheckCounts, CheckRun, CheckStatus, Commit,
    Deployment, MetadataItem, PullRequest, ReactionCounts, Resource, ResourceId, ResourceIdError,
    ResourceKind, FILE_PATCH_CONTEXT_UNAVAILABLE_WARNING, FULL_DEPTH_WARNING_HINT,
};
