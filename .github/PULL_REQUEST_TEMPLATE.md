name: Pull Request
description: Template for pull requests
title: "feat: "
labels: [triage]

body:
  - type: markdown
    attributes:
      value: |
        ## Thank you for contributing to FlowLens!
        Please fill out the following information to help us review your PR.

  - type: checkboxes
    id: checklist
    attributes:
      label: Checklist
      options:
        - label: I have read the [Contributing Guide](CONTRIBUTING.md)
          required: true
        - label: I have linked an issue (if applicable)
          required: false
        - label: My code follows the project's style guidelines
          required: true
        - label: I have added tests for my changes
          required: false
        - label: I have updated documentation as needed
          required: false

  - type: input
    id: type
    attributes:
      label: Type of change
      description: What type of change does your PR introduce?
      placeholder: e.g., bug fix, feature, refactor, docs
    validations:
      required: true

  - type: textarea
    id: description
    attributes:
      label: Description
      description: A clear and concise description of what your PR changes. Include motivation and context.
    validations:
      required: true

  - type: textarea
    id: related-issue
    attributes:
      label: Related Issue
      description: Links to related GitHub issues (e.g., Fixes #123)
      placeholder: Fixes #
    validations:
      required: false

  - type: textarea
    id: testing
    attributes:
      label: Testing
      description: How did you test this change? Include commands and test results.
      render: shell
      placeholder: |
        cargo test --workspace
        cargo clippy --workspace --all-targets -- -D warnings
    validations:
      required: true

  - type: textarea
    id: screenshots
    attributes:
      label: Screenshots (if appropriate)
      description: Add screenshots of UI changes
      render: markdown
    validations:
      required: false