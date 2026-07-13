# Synthetic WSJT-X UDP fixtures

`schema3-live-sequence.hex` contains synthetic big-endian QDataStream datagrams
for a heartbeat, status update, and WSPR decode. They were purpose-built from
the message layouts documented in the official WSJT-X
`Network/NetworkMessage.hpp` protocol source; they are not captured operator
traffic and contain no personal data.

The status and decode messages include trailing bytes intentionally. This pins
the protocol's forward-compatibility rule without treating those bytes as a
new AntennaBench schema.
