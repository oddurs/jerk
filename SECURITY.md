# Security policy

Please do not open a public issue for a vulnerability or accidentally exposed credential. Use GitHub's private vulnerability reporting for this repository instead.

jerk invokes `git`, and optionally `gh` and `curl`, without a shell. Repository paths and URLs are passed as individual process arguments. The future OpenRouter integration remains out of scope until its outbound payload is inspectable and its credential handling is isolated from repository configuration and logs.
