pub const viewer =
    \\query { viewer { id displayName email } }
;

pub const issue_list =
    \\query IssueList($filter: IssueFilter, $first: Int, $after: String) {
    \\  issues(filter: $filter, first: $first, after: $after, orderBy: updatedAt) {
    \\    nodes {
    \\      identifier
    \\      title
    \\      state { name color }
    \\      priority
    \\      priorityLabel
    \\      assignee { displayName }
    \\      labels { nodes { name } }
    \\      updatedAt
    \\    }
    \\    pageInfo { hasNextPage endCursor }
    \\  }
    \\}
;

pub const issue_view =
    \\query IssueView($filter: IssueFilter) {
    \\  issues(filter: $filter, first: 1) {
    \\    nodes {
    \\      identifier
    \\      title
    \\      description
    \\      state { name color }
    \\      priority
    \\      priorityLabel
    \\      team { name key }
    \\      assignee { displayName email }
    \\      project { name }
    \\      labels { nodes { name color } }
    \\      dueDate
    \\      estimate
    \\      comments(first: 50) {
    \\        nodes {
    \\          body
    \\          createdAt
    \\          user { displayName }
    \\        }
    \\      }
    \\    }
    \\  }
    \\}
;
