# Connector + Envoy Integration (Datum Connect)

## Purpose
This document captures how Connector and ConnectorAdvertisement will be used to
route traffic through Envoy and a local iroh sidecar. It is intended to live in
this repo as a reference for how datum-connect participates in the overall flow.

## Current Behavior (Datum Connect)
- A local tunnel is represented as an `Advertisment` with a TCP host/port.
- The listen node publishes a ticket to n0des for each enabled tunnel.
- The gateway extracts a subdomain, fetches the ticket, and forwards via iroh
  `ConnectTunnel` to the advertised host:port.

This is a useful baseline, but connectors move discovery and advertisement into
the control plane.

## Target Behavior (Connector-based)
### Control Plane Resources
- **Connector**: represents a running connector instance and its capabilities.
  - Connection details (public key, relay, addresses) are stored in
    `status.connectionDetails`.
  - The device agent writes `status.connectionDetails`; the control plane
    controller consumes it to publish discovery records.
- **ConnectorAdvertisement**: describes the destinations reachable via the
  connector.
  - For MVP, we only use `spec.layer4` (TCP/UDP services).

### Envoy + Sidecar Data Path
1) Envoy receives a request from the client.
2) Envoy routes the request to a local sidecar backend (not the real target).
3) The sidecar reads per-request metadata that identifies:
   - Connector NodeID (iroh public key)
   - Destination host
   - Destination port
   - Destination protocol (tcp/udp/ip)
4) The sidecar dials the remote connector using iroh and forwards traffic.

## Sidecar Responsibilities
The iroh sidecar running alongside Envoy must:
- Maintain an iroh endpoint and connectivity state.
- Accept local connections from Envoy (HTTP CONNECT, CONNECT-UDP, CONNECT-IP).
- Resolve the connector NodeID to connection details from the control plane
  (public key, relay, addresses) or dial by NodeID via discovery.
Potentially(todo(zach): find out where else this might be enforced):
- Enforce the allowed destination set from ConnectorAdvertisement.
- Gate functionality by connector capability status.

## How Envoy is Programmed
NSO will convert Datum CRDs into Gateway API resources, then apply
EnvoyPatchPolicies to inject per-route metadata for the sidecar. The key idea:
the EndpointSlice points to the local sidecar, not the real backend.
CONNECT must be explicitly enabled on the Gateway via
`gateway.envoyproxy.io/v1alpha1 BackendTrafficPolicy` with `httpUpgrade: CONNECT`.
With that enabled, a single HTTP listener can serve all tunnels with per-route
metadata/header injection.

### Route Metadata Contract (proposed)
Envoy will attach either request headers or route metadata for the sidecar to
consume. For example:
- `datum-node-id`: Connector NodeID (iroh public key)
- `datum-target-host`: backend host
- `datum-target-port`: backend port
- `datum-target-proto`: tcp | udp

These values are derived from HTTPProxy backend `endpoint` and connector
references.

## Discovery Publishing
The device agent updates `status.connectionDetails` with its NodeID (public key),
home relay, and observed addresses. A control plane controller publishes DNS TXT
records (or equivalent discovery data) from those details so peers can dial by
NodeID.

## Validation Rules
During reconciliation:
- A backend endpoint must match a ConnectorAdvertisement entry for the selected
  connector(s).
- If multiple connectors match a selector, the selection policy must be defined
  (single preferred, round-robin, or fail with conflict). For MVP, choose a
  single connector and emit a condition when ambiguous.
- Capability checks must pass (for example CONNECT-UDP requires capability).

## Open Questions
- How to select among multiple connectors matching a selector.

## Scope Note
For the initial implementation, we only support Layer 4 advertisements and
CONNECT-TCP. Layer 3 CIDRs and CONNECT-IP are deferred.

