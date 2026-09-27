# Takoda Desktop Infra

Single-host infrastructure and desired-state authority for running Takoda automation on a developer- or end-user-owned laptop/desktop.

This repository is intentionally separate from the hosted Takoda control plane. `tkda-desktop-daemon` is the machine-local lifecycle authority; `tkda-cli` and desktop/mobile clients are control-plane clients rather than direct process owners.

Implementation is developed through pull requests from this bootstrap commit.
