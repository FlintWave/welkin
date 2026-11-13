## ADDED Requirements

### Requirement: Message compatibility
The server SHALL encode and decode every message in the protocol message template byte-for-byte as OpenSimulator 0.9.3.0 does.

#### Scenario: Recorded session
- **WHEN** the packets of a recorded C# session are decoded and re-encoded
- **THEN** the output bytes equal the input bytes

### Requirement: LLSD encodings
The server SHALL read and write LLSD in XML, binary and notation form.

#### Scenario: Round trip
- **WHEN** an LLSD document is parsed and serialised in the same encoding
- **THEN** the result parses to an equal document

### Requirement: Reliable circuit
The LLUDP circuit SHALL resend unacknowledged reliable packets and SHALL enforce per-category throttles requested by the viewer.

#### Scenario: Lost packet
- **WHEN** a reliable packet is not acknowledged within the resend interval
- **THEN** it is sent again with the resent flag set

### Requirement: Malformed input is rejected safely
The decoders MUST return an error for malformed input and MUST NOT panic.

#### Scenario: Truncated packet
- **WHEN** a truncated packet arrives
- **THEN** it is dropped and counted, and the circuit stays open
