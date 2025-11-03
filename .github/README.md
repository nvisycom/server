# GitHub Workflows Documentation

This directory contains CI/CD workflows, custom actions, and automation for the nvisy-server project.

## Workflows

### Build & Test (`build.yml`)
Runs on every push and pull request to `main` and `release` branches.

**Jobs:**
- Code quality checks (formatting, linting)
- Cross-platform testing
- Code coverage analysis
- Performance benchmarks
- Integration tests

### Security (`security.yml`)
Runs daily and on code changes.

**Jobs:**
- Dependency vulnerability scanning
- License compliance checking
- Code security analysis
- Secret detection
- Container vulnerability scanning

### Release (`release.yml`)
Triggered by version tags (`v*`).

**Jobs:**
- Multi-platform binary compilation
- Docker image building and publishing
- Security scanning
- GitHub release creation
- crates.io publishing

---

## Artifact Retention Policy

Artifacts are temporary files generated during workflow runs. We follow these retention periods:

### Build Artifacts
| Artifact Type | Retention | Rationale |
|--------------|-----------|-----------|
| Coverage reports | 30 days | Historical trend analysis |
| Performance results | 30 days | Regression tracking |
| Build binaries | Not stored | Use GitHub Releases instead |

### Security Artifacts
| Artifact Type | Retention | Rationale |
|--------------|-----------|-----------|
| Security scan results | 30 days | Trend analysis |
| Security assessments | 90 days | Compliance/audit requirements |
| Vulnerability reports | 30 days | Historical tracking |

### Release Artifacts
| Artifact Type | Retention | Rationale |
|--------------|-----------|-----------|
| Temporary build artifacts | 7 days | Short-lived, moved to releases |
| GitHub Release assets | Permanent | Attached to releases |
| Container images | Permanent | Stored in GHCR |

**Cleanup:** Old artifacts are automatically deleted according to retention policies. Untagged container images are cleaned up after releases.

---

## Timeout Strategy

Workflow jobs have timeouts to prevent hanging builds and conserve CI resources.

### Build & Test Workflow
| Job | Timeout | Reason |
|-----|---------|--------|
| Code Quality Checks | 30 min | fmt, clippy, doc, dependency checks |
| Test Suite | 45 min | Matrix tests across multiple OS/Rust versions |
| Code Coverage | 30 min | Coverage instrumentation and report generation |
| Performance Tests | 45 min | Benchmark execution and regression analysis |
| Integration Tests | 20 min | E2E tests with server startup |

### Security Workflow
| Job | Timeout | Reason |
|-----|---------|--------|
| Dependency Security | 20 min | cargo-audit, cargo-deny, cargo-outdated |
| License Compliance | 15 min | Generate and check license report |
| Code Security Analysis | 25 min | Clippy security lints + Semgrep |
| Secret Detection | 15 min | TruffleHog and Gitleaks scanning |
| Container Security | 20 min | Build image and run Trivy scan |

### Release Workflow
| Job | Timeout | Reason |
|-----|---------|--------|
| Build Binaries | 60 min | Cross-compilation for 6 platforms (slow) |
| Build Docker | 30 min | Build and push container image |
| Security Scan | 15 min | Trivy vulnerability scan |
| Create Release | 15 min | Upload artifacts and create GitHub release |
| Publish Crates | 15 min | Publish to crates.io registry |
| Cleanup | 10 min | Delete artifacts and old images |

**Note:** Timeouts are set conservatively to account for variable CI runner performance and network conditions.

---

## Triggering Workflows

### Automatic Triggers

**Push or pull requests to `main` or `release`:**
- Build & Test workflow
- Security workflow

**Version tags (`v*`):**
- Release workflow

**Scheduled:**
- Build: Daily at 02:00 UTC
- Security: Daily at 01:00 UTC

### Manual Triggers

All workflows support `workflow_dispatch` for manual runs via GitHub UI.

**Build workflow options:**
- Run full test matrix (all OS/Rust versions)
- Run performance tests

**Security workflow options:**
- Select scan type: full, dependencies, code, secrets, containers, compliance

**Release workflow:**
- Dry run mode (test without publishing)

## Additional Resources

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Rust CI Best Practices](https://doc.rust-lang.org/cargo/guide/continuous-integration.html)
- [Security Scanning Guide](https://docs.github.com/en/code-security)
