use crate::domain::ResourceKind;

pub(crate) fn base_pr_query() -> &'static str {
    r#"
query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      number
      title
      url
      state
      author { login }
      createdAt
      updatedAt
      labels(first: 100) { nodes { name } }
      assignees(first: 100) { nodes { login name } }
      reactionGroups { content users { totalCount } }
      body
      baseRefName
      headRefName
      baseRefOid
      headRefOid
      headRepository { name nameWithOwner }
      headRepositoryOwner { login }
      reviewDecision
      reviewRequests(first: 100) {
        nodes {
          requestedReviewer {
            __typename
            ... on User { login name }
            ... on Team { name slug }
          }
        }
      }
      closingIssuesReferences(first: 100) { nodes { number url } }
      mergeStateStatus
      mergeable
      isDraft
      isCrossRepository
      maintainerCanModify
      changedFiles
      closed
      closedAt
      mergedAt
      mergedBy { login }
      milestone { title }
      autoMergeRequest { enabledAt }
      mergeCommit { oid }
      potentialMergeCommit { oid }
      additions
      deletions
      commits(first: 100) {
        nodes {
          commit {
            oid
            messageHeadline
            messageBody
            committedDate
            authoredDate
            authors(first: 100) {
              pageInfo {
                hasNextPage
                endCursor
              }
              nodes {
                name
                user { login name }
              }
            }
          }
        }
      }
      statusCheckRollup {
        contexts(first: 100) {
          nodes {
            __typename
            ... on CheckRun {
              name
              status
              conclusion
              detailsUrl
              startedAt
              completedAt
              checkSuite { workflowRun { workflow { name } } }
            }
            ... on StatusContext {
              context
              state
              targetUrl
            }
          }
        }
      }
      files(first: 100) { nodes { path additions deletions changeType } }
      comments(first: 100) {
        nodes {
          id
          author { login }
          authorAssociation
          body
          createdAt
          updatedAt
          url
          includesCreatedEdit
          isMinimized
          minimizedReason
          reactionGroups { content users { totalCount } }
        }
      }
      reviews(first: 100) {
        nodes {
          id
          author { login }
          authorAssociation
          body
          state
          submittedAt
          updatedAt
          url
          reactionGroups { content users { totalCount } }
        }
      }
    }
  }
}
"#
}

pub(crate) fn base_issue_query() -> &'static str {
    r#"
query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    issue(number: $number) {
      number
      title
      url
      state
      author { login }
      createdAt
      updatedAt
      labels(first: 100) { nodes { name } }
      assignees(first: 100) { nodes { login name } }
      reactionGroups { content users { totalCount } }
      body
      closed
      isPinned
      stateReason
      closedAt
      milestone { title }
      closedByPullRequestsReferences(first: 100) { nodes { number url } }
      comments(first: 100) {
        nodes {
          id
          author { login }
          authorAssociation
          body
          createdAt
          updatedAt
          url
          includesCreatedEdit
          isMinimized
          minimizedReason
          reactionGroups { content users { totalCount } }
        }
      }
    }
  }
}
"#
}

pub(crate) fn project_items_query(kind: ResourceKind) -> String {
    let selector = selector(kind);
    format!(
        r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {{
  repository(owner: $owner, name: $name) {{
    {selector}(number: $number) {{
      projectItems(first: 100, after: $after) {{
        pageInfo {{
          hasNextPage
          endCursor
        }}
        nodes {{
          project {{
            title
          }}
        }}
      }}
    }}
  }}
}}
"#
    )
}

pub(crate) fn labels_query(kind: ResourceKind) -> String {
    let selector = selector(kind);
    format!(
        r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {{
  repository(owner: $owner, name: $name) {{
    {selector}(number: $number) {{
      labels(first: 100, after: $after) {{
        pageInfo {{
          hasNextPage
          endCursor
        }}
        nodes {{
          name
        }}
      }}
    }}
  }}
}}
"#
    )
}

pub(crate) fn assignees_query(kind: ResourceKind) -> String {
    let selector = selector(kind);
    format!(
        r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {{
  repository(owner: $owner, name: $name) {{
    {selector}(number: $number) {{
      assignees(first: 100, after: $after) {{
        pageInfo {{
          hasNextPage
          endCursor
        }}
        nodes {{
          login
          name
        }}
      }}
    }}
  }}
}}
"#
    )
}

pub(crate) fn review_requests_query() -> &'static str {
    r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      reviewRequests(first: 100, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          requestedReviewer {
            __typename
            ... on User { login name }
            ... on Team { name slug }
          }
        }
      }
    }
  }
}
"#
}

pub(crate) fn linked_resources_query(kind: ResourceKind) -> String {
    let selector = selector(kind);
    let connection = match kind {
        ResourceKind::Issue => "closedByPullRequestsReferences",
        ResourceKind::PullRequest => "closingIssuesReferences",
    };
    format!(
        r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {{
  repository(owner: $owner, name: $name) {{
    {selector}(number: $number) {{
      {connection}(first: 100, after: $after) {{
        pageInfo {{
          hasNextPage
          endCursor
        }}
        nodes {{
          number
          url
        }}
      }}
    }}
  }}
}}
"#
    )
}

pub(crate) fn comments_query(kind: ResourceKind) -> String {
    let selector = selector(kind);
    format!(
        r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {{
  repository(owner: $owner, name: $name) {{
    {selector}(number: $number) {{
      comments(first: 100, after: $after) {{
        pageInfo {{
          hasNextPage
          endCursor
        }}
        nodes {{
          id
          author {{ login }}
          authorAssociation
          body
          createdAt
          updatedAt
          url
          includesCreatedEdit
          isMinimized
          minimizedReason
          reactionGroups {{
            content
            users {{ totalCount }}
          }}
        }}
      }}
    }}
  }}
}}
"#
    )
}

pub(crate) fn commits_query() -> &'static str {
    r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      commits(first: 100, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          commit {
            oid
            messageHeadline
            messageBody
            committedDate
            authoredDate
            authors(first: 100) {
              pageInfo {
                hasNextPage
                endCursor
              }
              nodes {
                name
                user { login name }
              }
            }
          }
        }
      }
    }
  }
}
"#
}

pub(crate) fn commit_authors_query() -> &'static str {
    r#"
query($owner: String!, $name: String!, $oid: GitObjectID!, $after: String) {
  repository(owner: $owner, name: $name) {
    object(oid: $oid) {
      ... on Commit {
        authors(first: 100, after: $after) {
          pageInfo {
            hasNextPage
            endCursor
          }
          nodes {
            name
            user { login name }
          }
        }
      }
    }
  }
}
"#
}

pub(crate) fn review_threads_query() -> &'static str {
    r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      reviewThreads(first: 100, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          id
          isResolved
          isOutdated
          path
          line
          comments(first: 100) {
            pageInfo {
              hasNextPage
              endCursor
            }
            nodes {
              id
              author { login }
              authorAssociation
              body
              createdAt
              updatedAt
              url
              includesCreatedEdit
              isMinimized
              minimizedReason
              reactionGroups {
                content
                users { totalCount }
              }
              path
              line
            }
          }
        }
      }
    }
  }
}
"#
}

pub(crate) fn reviews_query() -> &'static str {
    r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      reviews(first: 100, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          id
          author { login }
          authorAssociation
          body
          state
          submittedAt
          updatedAt
          url
          reactionGroups {
            content
            users { totalCount }
          }
        }
      }
    }
  }
}
"#
}

pub(crate) fn review_thread_comments_query() -> &'static str {
    r#"
query($threadId: ID!, $after: String) {
  node(id: $threadId) {
    ... on PullRequestReviewThread {
      comments(first: 100, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          id
          author { login }
          authorAssociation
          body
          createdAt
          updatedAt
          url
          includesCreatedEdit
          isMinimized
          minimizedReason
          reactionGroups {
            content
            users { totalCount }
          }
          path
          line
        }
      }
    }
  }
}
"#
}

pub(crate) fn commit_comment_thread_comments_query() -> &'static str {
    r#"
query($threadId: ID!, $after: String) {
  node(id: $threadId) {
    ... on PullRequestCommitCommentThread {
      comments(first: 100, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          id
          author { login }
          authorAssociation
          body
          createdAt
          updatedAt
          url
          includesCreatedEdit
          isMinimized
          minimizedReason
          reactionGroups {
            content
            users { totalCount }
          }
          path
          position
        }
      }
    }
  }
}
"#
}

pub(crate) fn changed_files_query() -> &'static str {
    r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      files(first: 100, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          path
          additions
          deletions
          changeType
        }
      }
    }
  }
}
"#
}

pub(crate) fn timeline_query(kind: ResourceKind) -> String {
    let selector = selector(kind);
    let pr_timeline_items = match kind {
        ResourceKind::PullRequest => {
            r#",
        MERGED_EVENT,
        REVIEW_REQUESTED_EVENT,
        REVIEW_REQUEST_REMOVED_EVENT,
        READY_FOR_REVIEW_EVENT,
        CONVERT_TO_DRAFT_EVENT,
        BASE_REF_CHANGED_EVENT,
        BASE_REF_FORCE_PUSHED_EVENT,
        BASE_REF_DELETED_EVENT,
        HEAD_REF_FORCE_PUSHED_EVENT,
        HEAD_REF_DELETED_EVENT,
        HEAD_REF_RESTORED_EVENT,
        REVIEW_DISMISSED_EVENT,
        ADDED_TO_MERGE_QUEUE_EVENT,
        REMOVED_FROM_MERGE_QUEUE_EVENT,
        AUTOMATIC_BASE_CHANGE_FAILED_EVENT,
        AUTOMATIC_BASE_CHANGE_SUCCEEDED_EVENT,
        AUTO_REBASE_ENABLED_EVENT,
        AUTO_SQUASH_ENABLED_EVENT,
        AUTO_MERGE_ENABLED_EVENT,
        AUTO_MERGE_DISABLED_EVENT,
        PULL_REQUEST_COMMIT_COMMENT_THREAD,
        DEPLOYED_EVENT,
        DEPLOYMENT_ENVIRONMENT_CHANGED_EVENT"#
        }
        ResourceKind::Issue => "",
    };
    let pr_timeline_fragments = match kind {
        ResourceKind::PullRequest => {
            r#"
          ... on MergedEvent {
            id
            createdAt
            actor { login }
            mergeRefName
            commit { oid }
          }
          ... on ReviewRequestedEvent {
            id
            createdAt
            actor { login }
            requestedReviewer { __typename ... on User { login } ... on Team { name slug } }
          }
          ... on ReviewRequestRemovedEvent {
            id
            createdAt
            actor { login }
            requestedReviewer { __typename ... on User { login } ... on Team { name slug } }
          }
          ... on ReadyForReviewEvent { id createdAt actor { login } }
          ... on ConvertToDraftEvent { id createdAt actor { login } }
          ... on BaseRefChangedEvent { id createdAt actor { login } previousRefName currentRefName }
          ... on BaseRefForcePushedEvent {
            id
            createdAt
            actor { login }
            beforeCommit { oid }
            afterCommit { oid }
            ref { name }
          }
          ... on BaseRefDeletedEvent { id createdAt actor { login } baseRefName }
          ... on HeadRefForcePushedEvent {
            id
            createdAt
            actor { login }
            beforeCommit { oid }
            afterCommit { oid }
            ref { name }
          }
          ... on HeadRefDeletedEvent { id createdAt actor { login } headRefName }
          ... on HeadRefRestoredEvent { id createdAt actor { login } }
          ... on ReviewDismissedEvent {
            id
            createdAt
            actor { login }
            previousReviewState
            dismissalMessage
            url
          }
          ... on AddedToMergeQueueEvent {
            id
            createdAt
            actor { login }
            enqueuer { login }
          }
          ... on RemovedFromMergeQueueEvent {
            id
            createdAt
            actor { login }
            enqueuer { login }
            beforeCommit { oid }
            reason
          }
          ... on AutomaticBaseChangeFailedEvent { id createdAt actor { login } oldBase newBase }
          ... on AutomaticBaseChangeSucceededEvent { id createdAt actor { login } oldBase newBase }
          ... on AutoRebaseEnabledEvent { id createdAt actor { login } enabler { login } }
          ... on AutoSquashEnabledEvent { id createdAt actor { login } enabler { login } }
          ... on AutoMergeEnabledEvent { id createdAt actor { login } }
          ... on AutoMergeDisabledEvent { id createdAt actor { login } reason }
          ... on PullRequestCommitCommentThread {
            id
            path
            position
            commit { oid }
            comments(first: 100) {
              pageInfo {
                hasNextPage
                endCursor
              }
              nodes {
                id
                author { login }
                authorAssociation
                body
                createdAt
                updatedAt
                url
                includesCreatedEdit
                isMinimized
                minimizedReason
                reactionGroups {
                  content
                  users { totalCount }
                }
                path
                position
              }
            }
          }
          ... on DeployedEvent {
            id
            createdAt
            actor { login }
            ref { name }
            deployment {
              environment
              latestEnvironment
              state
              latestStatus {
                state
                environmentUrl
                logUrl
              }
            }
          }
          ... on DeploymentEnvironmentChangedEvent {
            id
            createdAt
            actor { login }
            deploymentStatus {
              state
              environment
              environmentUrl
              logUrl
              deployment {
                environment
                latestEnvironment
              }
            }
          }"#
        }
        ResourceKind::Issue => "",
    };
    format!(
        r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {{
  repository(owner: $owner, name: $name) {{
    {selector}(number: $number) {{
      timelineItems(first: 100, after: $after, itemTypes: [
        CLOSED_EVENT,
        REOPENED_EVENT,
        LABELED_EVENT,
        UNLABELED_EVENT,
        ASSIGNED_EVENT,
        UNASSIGNED_EVENT,
        PINNED_EVENT,
        UNPINNED_EVENT,
        LOCKED_EVENT,
        UNLOCKED_EVENT,
        ADDED_TO_PROJECT_EVENT,
        ADDED_TO_PROJECT_V2_EVENT,
        MOVED_COLUMNS_IN_PROJECT_EVENT,
        REMOVED_FROM_PROJECT_EVENT,
        REMOVED_FROM_PROJECT_V2_EVENT,
        PROJECT_V2_ITEM_STATUS_CHANGED_EVENT,
        CONVERTED_FROM_DRAFT_EVENT,
        CONVERTED_NOTE_TO_ISSUE_EVENT,
        USER_BLOCKED_EVENT,
        SUBSCRIBED_EVENT,
        UNSUBSCRIBED_EVENT,
        MENTIONED_EVENT,
        COMMENT_DELETED_EVENT,
        TRANSFERRED_EVENT,
        MARKED_AS_DUPLICATE_EVENT,
        UNMARKED_AS_DUPLICATE_EVENT,
        CONNECTED_EVENT,
        DISCONNECTED_EVENT,
        REFERENCED_EVENT,
        CROSS_REFERENCED_EVENT,
        RENAMED_TITLE_EVENT,
        MILESTONED_EVENT,
        DEMILESTONED_EVENT,
        CONVERTED_TO_DISCUSSION_EVENT,
        ISSUE_COMMENT_PINNED_EVENT,
        ISSUE_COMMENT_UNPINNED_EVENT,
        ISSUE_TYPE_ADDED_EVENT,
        ISSUE_TYPE_REMOVED_EVENT,
        ISSUE_TYPE_CHANGED_EVENT,
        ISSUE_FIELD_ADDED_EVENT,
        ISSUE_FIELD_REMOVED_EVENT,
        ISSUE_FIELD_CHANGED_EVENT,
        SUB_ISSUE_ADDED_EVENT,
        SUB_ISSUE_REMOVED_EVENT,
        PARENT_ISSUE_ADDED_EVENT,
        PARENT_ISSUE_REMOVED_EVENT,
        BLOCKED_BY_ADDED_EVENT,
        BLOCKED_BY_REMOVED_EVENT,
        BLOCKING_ADDED_EVENT,
        BLOCKING_REMOVED_EVENT{pr_timeline_items}
      ]) {{
        pageInfo {{
          hasNextPage
          endCursor
        }}
        nodes {{
          __typename
          ... on ClosedEvent {{ id createdAt actor {{ login }} closer {{ __typename }} }}
          ... on ReopenedEvent {{ id createdAt actor {{ login }} }}
          ... on LabeledEvent {{ id createdAt actor {{ login }} label {{ name }} }}
          ... on UnlabeledEvent {{ id createdAt actor {{ login }} label {{ name }} }}
          ... on AssignedEvent {{
            id
            createdAt
            actor {{ login }}
            assignee {{ __typename ... on User {{ login }} }}
          }}
          ... on UnassignedEvent {{
            id
            createdAt
            actor {{ login }}
            assignee {{ __typename ... on User {{ login }} }}
          }}
          ... on PinnedEvent {{ id createdAt actor {{ login }} }}
          ... on UnpinnedEvent {{ id createdAt actor {{ login }} }}
          ... on LockedEvent {{ id createdAt actor {{ login }} lockReason }}
          ... on UnlockedEvent {{ id createdAt actor {{ login }} }}
          ... on AddedToProjectEvent {{ id createdAt actor {{ login }} }}
          ... on AddedToProjectV2Event {{ id createdAt actor {{ login }} wasAutomated }}
          ... on MovedColumnsInProjectEvent {{ id createdAt actor {{ login }} }}
          ... on RemovedFromProjectEvent {{ id createdAt actor {{ login }} }}
          ... on RemovedFromProjectV2Event {{ id createdAt actor {{ login }} wasAutomated }}
          ... on ProjectV2ItemStatusChangedEvent {{
            id
            createdAt
            actor {{ login }}
            previousStatus
            status
            wasAutomated
          }}
          ... on ConvertedFromDraftEvent {{ id createdAt actor {{ login }} wasAutomated }}
          ... on ConvertedNoteToIssueEvent {{ id createdAt actor {{ login }} projectColumnName }}
          ... on UserBlockedEvent {{
            id
            createdAt
            actor {{ login }}
            blockDuration
            subject {{ login }}
          }}
          ... on SubscribedEvent {{ id createdAt actor {{ login }} }}
          ... on UnsubscribedEvent {{ id createdAt actor {{ login }} }}
          ... on MentionedEvent {{ id createdAt actor {{ login }} }}
          ... on CommentDeletedEvent {{ id createdAt actor {{ login }} }}
          ... on TransferredEvent {{
            id
            createdAt
            actor {{ login }}
            fromRepository {{ nameWithOwner }}
          }}
          ... on MarkedAsDuplicateEvent {{
            id
            createdAt
            actor {{ login }}
            canonical {{ __typename ... on Issue {{ number title url repository {{ nameWithOwner }} }} ... on PullRequest {{ number title url repository {{ nameWithOwner }} }} }}
            duplicate {{ __typename ... on Issue {{ number title url repository {{ nameWithOwner }} }} ... on PullRequest {{ number title url repository {{ nameWithOwner }} }} }}
          }}
          ... on UnmarkedAsDuplicateEvent {{
            id
            createdAt
            actor {{ login }}
            canonical {{ __typename ... on Issue {{ number title url repository {{ nameWithOwner }} }} ... on PullRequest {{ number title url repository {{ nameWithOwner }} }} }}
            duplicate {{ __typename ... on Issue {{ number title url repository {{ nameWithOwner }} }} ... on PullRequest {{ number title url repository {{ nameWithOwner }} }} }}
          }}
          ... on ConnectedEvent {{
            id
            createdAt
            actor {{ login }}
            source {{ __typename ... on Issue {{ number title url repository {{ nameWithOwner }} }} ... on PullRequest {{ number title url repository {{ nameWithOwner }} }} }}
            subject {{ __typename ... on Issue {{ number title url repository {{ nameWithOwner }} }} ... on PullRequest {{ number title url repository {{ nameWithOwner }} }} }}
          }}
          ... on DisconnectedEvent {{
            id
            createdAt
            actor {{ login }}
            source {{ __typename ... on Issue {{ number title url repository {{ nameWithOwner }} }} ... on PullRequest {{ number title url repository {{ nameWithOwner }} }} }}
            subject {{ __typename ... on Issue {{ number title url repository {{ nameWithOwner }} }} ... on PullRequest {{ number title url repository {{ nameWithOwner }} }} }}
          }}
          ... on ReferencedEvent {{ id createdAt actor {{ login }} commit {{ oid }} }}
          ... on CrossReferencedEvent {{
            id
            createdAt
            actor {{ login }}
            source {{
              __typename
              ... on Issue {{ number title url repository {{ nameWithOwner }} }}
              ... on PullRequest {{ number title url repository {{ nameWithOwner }} }}
            }}
          }}
          ... on RenamedTitleEvent {{ id createdAt actor {{ login }} previousTitle currentTitle }}
          ... on MilestonedEvent {{ id createdAt actor {{ login }} milestoneTitle }}
          ... on DemilestonedEvent {{ id createdAt actor {{ login }} milestoneTitle }}
          ... on ConvertedToDiscussionEvent {{
            id
            createdAt
            actor {{ login }}
            discussion {{ title url }}
          }}
          ... on IssueCommentPinnedEvent {{
            id
            createdAt
            actor {{ login }}
            issueComment {{ url }}
          }}
          ... on IssueCommentUnpinnedEvent {{
            id
            createdAt
            actor {{ login }}
            issueComment {{ url }}
          }}
          ... on IssueTypeAddedEvent {{
            id
            createdAt
            actor {{ login }}
            issueType {{ name }}
          }}
          ... on IssueTypeRemovedEvent {{
            id
            createdAt
            actor {{ login }}
            issueType {{ name }}
          }}
          ... on IssueTypeChangedEvent {{
            id
            createdAt
            actor {{ login }}
            prevIssueType {{ name }}
            issueType {{ name }}
          }}
          ... on IssueFieldAddedEvent {{
            id
            createdAt
            actor {{ login }}
            value
            color
            issueField {{
              __typename
              ... on IssueFieldDate {{ name }}
              ... on IssueFieldNumber {{ name }}
              ... on IssueFieldSingleSelect {{ name }}
              ... on IssueFieldText {{ name }}
            }}
          }}
          ... on IssueFieldRemovedEvent {{
            id
            createdAt
            actor {{ login }}
            issueField {{
              __typename
              ... on IssueFieldDate {{ name }}
              ... on IssueFieldNumber {{ name }}
              ... on IssueFieldSingleSelect {{ name }}
              ... on IssueFieldText {{ name }}
            }}
          }}
          ... on IssueFieldChangedEvent {{
            id
            createdAt
            actor {{ login }}
            previousValue
            newValue
            previousColor
            newColor
            issueField {{
              __typename
              ... on IssueFieldDate {{ name }}
              ... on IssueFieldNumber {{ name }}
              ... on IssueFieldSingleSelect {{ name }}
              ... on IssueFieldText {{ name }}
            }}
          }}
          ... on SubIssueAddedEvent {{
            id
            createdAt
            actor {{ login }}
            subIssue {{ number title url repository {{ nameWithOwner }} }}
          }}
          ... on SubIssueRemovedEvent {{
            id
            createdAt
            actor {{ login }}
            subIssue {{ number title url repository {{ nameWithOwner }} }}
          }}
          ... on ParentIssueAddedEvent {{
            id
            createdAt
            actor {{ login }}
            parent {{ number title url repository {{ nameWithOwner }} }}
          }}
          ... on ParentIssueRemovedEvent {{
            id
            createdAt
            actor {{ login }}
            parent {{ number title url repository {{ nameWithOwner }} }}
          }}
          ... on BlockedByAddedEvent {{
            id
            createdAt
            actor {{ login }}
            blockingIssue {{ number title url repository {{ nameWithOwner }} }}
          }}
          ... on BlockedByRemovedEvent {{
            id
            createdAt
            actor {{ login }}
            blockingIssue {{ number title url repository {{ nameWithOwner }} }}
          }}
          ... on BlockingAddedEvent {{
            id
            createdAt
            actor {{ login }}
            blockedIssue {{ number title url repository {{ nameWithOwner }} }}
          }}
          ... on BlockingRemovedEvent {{
            id
            createdAt
            actor {{ login }}
            blockedIssue {{ number title url repository {{ nameWithOwner }} }}
          }}
          {pr_timeline_fragments}
        }}
      }}
    }}
  }}
}}
"#
    )
}

pub(crate) fn commit_deployments_query() -> &'static str {
    r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      commits(first: 100, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          commit {
            oid
            deployments(first: 100) {
              pageInfo {
                hasNextPage
                endCursor
              }
              nodes {
                environment
                task
                description
                createdAt
                updatedAt
                latestStatus {
                  state
                  description
                  environmentUrl
                  logUrl
                  createdAt
                }
              }
            }
          }
        }
      }
    }
  }
}
"#
}

pub(crate) fn commit_deployment_items_query() -> &'static str {
    r#"
query($owner: String!, $name: String!, $oid: GitObjectID!, $after: String) {
  repository(owner: $owner, name: $name) {
    object(oid: $oid) {
      ... on Commit {
        deployments(first: 100, after: $after) {
          pageInfo {
            hasNextPage
            endCursor
          }
          nodes {
            environment
            task
            description
            createdAt
            updatedAt
            latestStatus {
              state
              description
              environmentUrl
              logUrl
              createdAt
            }
          }
        }
      }
    }
  }
}
"#
}

pub(crate) fn check_suites_query() -> &'static str {
    r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      commits(last: 1) {
        nodes {
          commit {
            checkSuites(first: 100, after: $after) {
              pageInfo {
                hasNextPage
                endCursor
              }
              nodes {
                status
                conclusion
                url
                app { name }
                workflowRun {
                  url
                  workflow { name }
                }
              }
            }
          }
        }
      }
    }
  }
}
"#
}

pub(crate) fn status_rollup_query() -> &'static str {
    r#"
query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      statusCheckRollup {
        contexts(first: 100, after: $after) {
          pageInfo {
            hasNextPage
            endCursor
          }
          nodes {
            __typename
            ... on CheckRun {
              name
              status
              conclusion
              detailsUrl
              startedAt
              completedAt
              checkSuite { workflowRun { workflow { name } } }
            }
            ... on StatusContext {
              context
              state
              targetUrl
            }
          }
        }
      }
    }
  }
}
"#
}

fn selector(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::PullRequest => "pullRequest",
        ResourceKind::Issue => "issue",
    }
}
