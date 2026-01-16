# NSO Work Plan: Connector + Envoy Programming

## Goal
Add Connector and ConnectorAdvertisement support to the
network-services-operator (NSO) so HTTPProxy backends can be routed through a
connector and the Envoy sidecar can dial via iroh.

## Current NSO Behavior
- HTTPProxy reconciler creates Gateway + HTTPRoute + EndpointSlice from Datum
  HTTPProxy.
- TrafficProtectionPolicy creates EnvoyPatchPolicy to inject Coraza filters and
  metadata.
- Gateway-Envoy policy CRDs are mirrored to the downstream control plane.

## New Controllers
### 1) Connector Controller
Responsibilities:
- Watch `Connector` resources.
- Validate connector class existence.
- The connector agent is expected to set `status.connectionDetails` and
  capability conditions.
- Publish DNS TXT records (or equivalent discovery records) from
  `status.connectionDetails` for dial-by-NodeID.

### 2) ConnectorAdvertisement Controller
Responsibilities:
- Watch `ConnectorAdvertisement` resources.
- Validate `connectorRef` exists and is in the same namespace.
- Track advertisements for later HTTPProxy reconciliation.

## HTTPProxy Reconciliation Changes
### Selection
- Add support for `backend.connector` references in the Datum HTTPProxy API.
- Resolve connector by name or label selector.
- If multiple connectors match, apply a deterministic selection rule (or fail
  with a condition for MVP).

### Validation
- Ensure the backend endpoint matches an advertised L4 service in
  ConnectorAdvertisement.
- Validate protocol and port are allowed.
- Validate required connector capability (CONNECT-TCP/UDP).

### Gateway API Programming
- Keep Gateway + HTTPRoute creation as-is.
- For connector-backed routes, replace EndpointSlice host/port with the
  sidecar address (for example `127.0.0.1:<port>` or a Service backing the
  sidecar).
- Ensure CONNECT is enabled on the Gateway via
  `gateway.envoyproxy.io/v1alpha1 BackendTrafficPolicy` with
  `httpUpgrade: CONNECT` so a single listener can serve all tunnel routes.

### EnvoyPatchPolicy Programming
Create EnvoyPatchPolicy patches to inject metadata for the sidecar. Options:
- Add request headers (RequestHeaderModifier) on the HTTPRoute backend.
  - Tested in kind with Envoy Gateway: headers were preserved for HTTP and
    CONNECT once `BackendTrafficPolicy` enabled CONNECT.
- Add per-route metadata via JSONPatch (RouteConfiguration).

Recommended metadata keys:
- `datum-node-id`
- `datum-target-host`
- `datum-target-port`
- `datum-target-proto`

These values come from the original backend endpoint and the chosen connector.
The node ID is the connector's iroh public key (NodeID).

## Required API Updates
- Extend `HTTPProxyRuleBackend` to include `connector` (name or selector).
- Add validation to reject invalid combinations (for example connector +
  non-connectable protocols).

## Downstream Expectations
Envoy Gateway continues to watch Gateway API resources and EnvoyPatchPolicy.
NSO remains responsible for:
- Producing correct EndpointSlices
- Injecting metadata for the sidecar
- Surfacing errors via HTTPProxy conditions

## MVP Checklist
- Add `connector` fields to HTTPProxy API and validation.
- Implement Connector + ConnectorAdvertisement controllers.
- Update HTTPProxy controller to handle connector-backed routes:
  - select connector
  - validate advertisement
  - swap EndpointSlice destination to sidecar
  - attach metadata via EnvoyPatchPolicy or request headers
- Ensure BackendTrafficPolicy enables CONNECT on the Gateway.
- Define selection policy for multiple connector matches.
- Add conditions to HTTPProxy status for selection/validation errors.

