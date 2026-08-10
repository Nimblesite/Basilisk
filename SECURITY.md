# Security Policy

<!-- agent-pmo:0b21609 -->

## Supported Versions

**None.** Basilisk is unlisted and its type checker is inert
([the statement](https://www.basilisk-python.dev/)). No version receives
security fixes, and no version will. If Basilisk is still installed anywhere,
remove it — that is the only remediation this project can offer.

| Version | Supported |
| ------- | --------- |
| all     | ❌        |

## Reporting a Vulnerability

You can still reach us, and we would rather hear about a problem than not.

**Please do not report security vulnerabilities through public GitHub issues,
discussions, or pull requests.**

Report privately through GitHub's **private vulnerability reporting**: go to the
repository's **Security** tab → **Report a vulnerability** (or
<https://github.com/Nimblesite/Basilisk/security/advisories/new>). This opens a
private, structured advisory only the maintainers can see.

If you cannot use that channel, email **security@nimblesite.co**.

When reporting, please include:

- The type of issue (e.g. injection, path traversal, auth bypass, secret exposure).
- The affected version(s), file(s), and any relevant configuration.
- Steps to reproduce, ideally a minimal proof of concept.
- The impact: what an attacker can achieve.

## What to Expect

We are not promising a response window on an unlisted project, and we will not
be shipping a patched version. What a report can still achieve: a published
advisory, so anyone who has not yet removed Basilisk knows why they should.

## References

- Add a security policy: <https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/configure-vulnerability-reporting/add-security-policy>
- Configure private vulnerability reporting: <https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/configure-vulnerability-reporting/configure-for-a-repository>
