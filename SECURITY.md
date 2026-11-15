# Security Policy

## Supported versions

Only the latest release on `main` receives security fixes.

## Reporting a vulnerability

Email security concerns to the maintainers privately. Do not open a
public issue for security vulnerabilities.

Use GitHub's private vulnerability reporting:
**Security** tab > **Report a vulnerability**.

We aim to acknowledge reports within 48 hours and provide a fix or
mitigation plan within 7 days for confirmed issues.

## Scope

Welkin is a virtual world server that handles network connections from
untrusted viewers. Security-relevant areas include:

- LLUDP packet parsing and handling
- HTTP/HTTPS capability endpoints
- LSL/OSSL script execution sandbox
- Authentication and session management
- Asset and inventory storage
- Hypergrid federation trust boundaries
